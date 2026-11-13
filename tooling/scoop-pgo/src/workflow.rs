use std::process::{Command, Stdio};

use scoop::{
    BuildProfile,
    options::{BuildInvocation, CommandArgument, MessageFormat},
};
use scoop_process::CommandExt;
use serde_json::{Value, json};

use crate::{
    ExitStatus, Result,
    args::{self, Parsed},
    build,
    data::{self, Dataset, Run, RunStatus, SavedBuild},
    merge,
    process::Tools,
};

pub fn execute(parsed: Parsed) -> Result<Option<ExitStatus>> {
    let cwd = std::env::current_dir()?;
    let instrumenting = matches!(parsed.cli.command, args::Command::Instrument(_));
    let common = match &parsed.cli.command {
        args::Command::Instrument(args) => &args.common,
        args::Command::Optimize(args) => args,
    };
    args::validate_fixed_modes(common, &parsed.provided)?;
    let dataset = Dataset::open(cwd.join(&common.data_dir))?;
    let saved = dataset.saved()?;
    if !instrumenting && saved.is_none() {
        return Err("no saved training build; use instrument first".into());
    }
    let tools = Tools::resolve(&parsed.cli, saved.as_ref(), &cwd)?;
    let mut options = if let Some(saved) = &saved {
        args::overlay(
            &saved.invocation.options,
            common.build.clone(),
            &parsed.provided,
            &cwd,
        )?
    } else {
        let mut options = common.build.clone().normalized(&cwd)?;
        if !parsed.provided.contains("export_ir") {
            options.export_ir = scoop_protocol::IrTemplateExport::Full;
        }
        options
    };
    if options.packages.selects_members() && !parsed.provided.contains("root_input") {
        // An explicit member selection must use normal workspace discovery from this cwd.
        options.root_input = None;
    }
    if let Some(saved) = &saved
        && saved.driver.to_path_buf()? != tools.driver
        && !parsed.provided.contains("scoopc")
    {
        options.scoopc = Some(
            tools
                .driver
                .with_file_name(scoop_toolchain::host_executable_name("scoopc")),
        );
    }
    options.profile = Some(if instrumenting {
        BuildProfile::Profiling
    } else {
        BuildProfile::Release
    });
    options.release = false;
    options.profile_use = None;
    options.profile_use_required = !instrumenting;
    options.message_format = MessageFormat::Json;
    // Debug information defaults belong to the phase, not the preceding training build.
    if !parsed.provided.contains("debug_info") {
        options.debug_info = None;
    }
    let target = options
        .target
        .clone()
        .unwrap_or(scoop_toolchain::host_target_triple()?.to_owned());
    let compiler = options.scoopc.clone().unwrap_or_else(|| {
        tools
            .driver
            .with_file_name(scoop_toolchain::host_executable_name("scoopc"))
    });
    let gc_stress = parsed.cli.enable_gc_stress
        || saved
            .as_ref()
            .is_some_and(|saved| saved.invocation.enable_gc_stress);
    let startup_stats = parsed.cli.enable_startup_stats
        || saved
            .as_ref()
            .is_some_and(|saved| saved.invocation.enable_startup_stats);
    if !instrumenting {
        options.profile_use = Some(merge::update(&dataset, &tools, &compiler, &target)?);
    }
    let phase = if instrumenting {
        "instrumented"
    } else {
        "optimized"
    };
    let output = build::Output::prepare(common, &dataset, phase, &target, &cwd)?;
    let result = build::run(
        &tools,
        &options,
        common,
        &output,
        &cwd,
        gc_stress,
        startup_stats,
    )?;
    if let Some(saved) = &saved {
        compatible(&saved.invocation, &result.invocation)?;
    }
    if !instrumenting {
        let destination = output.publish()?;
        data::write_json(
            &dataset.root.join("optimization.json"),
            &json!({
                "schema": data::SCHEMA,
                "output": data::path(&destination)?,
                "invocation": result.invocation,
                "profile_use": result.record["profile_use"],
                "observations": result.record["observations"],
                "profile": "merged/profile.json", "summary": "merged/summary.json",
            }),
        )?;
        eprintln!(
            "Optimized {} using matching profile feedback",
            destination.display()
        );
        eprintln!(
            "Optimization details: {}",
            dataset.root.join("optimization.json").display()
        );
        return Ok(None);
    }
    if result.maps.is_empty() {
        return Err("training build did not produce any profile maps".into());
    }
    let maps = dataset.save_maps(&result.build_id, &result.maps)?;
    let destination = output.publish()?;
    let saved = SavedBuild {
        schema: data::SCHEMA,
        driver: data::path(&tools.driver)?,
        invocation: result.invocation,
        build_id: result.build_id,
        maps,
        output: data::path(&destination)?,
    };
    data::write_json(&dataset.root.join("build.json"), &saved)?;
    report_coverage(&result.record);
    let args::Command::Instrument(args) = parsed.cli.command else {
        unreachable!("instrument phase was selected above")
    };
    let directory = dataset.new_run()?;
    let mut run = Run {
        schema: data::SCHEMA,
        build_id: saved.build_id.clone(),
        maps: saved.maps.clone(),
        cwd: data::path(&cwd)?,
        arguments: args
            .program_args
            .iter()
            .map(|value| CommandArgument::from_os(value))
            .collect::<std::result::Result<_, _>>()?,
        status: None,
        raw: Vec::new(),
    };
    data::write_json(&directory.join("run.json"), &run)?;
    eprintln!("Instrumented {}", destination.display());
    if args.no_run {
        eprintln!(
            "For an external run, set SCOOP_PROFILE_DIR to {}",
            directory.display()
        );
        eprintln!(
            "Run the instrumented binary with the recorded program arguments, then use optimize --data-dir {}",
            dataset.root.display()
        );
        return Ok(None);
    }
    let status = Command::new(&destination)
        .args(&args.program_args)
        .current_dir(&cwd)
        .env("SCOOP_PROFILE_DIR", &directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .scoop_spawn()?
        .wait()?;
    run.status = Some(RunStatus::from_status(status));
    run.raw = data::raw_files(&directory)?;
    data::write_json(&directory.join("run.json"), &run)?;
    let collection = if run.raw.is_empty() {
        Err(
            "the program did not write runtime counters; inspect its profiling output diagnostic"
                .into(),
        )
    } else {
        let target = saved
            .invocation
            .options
            .target
            .as_deref()
            .ok_or("build result is missing its target")?;
        let compiler = saved
            .invocation
            .options
            .scoopc
            .as_deref()
            .ok_or("build result is missing its compiler")?;
        merge::update(&dataset, &tools, compiler, target).map(|_| ())
    };
    if !status.success() {
        if let Err(error) = collection {
            eprintln!("scoop-pgo: {error}");
        }
        eprintln!(
            "Training process exited with {status}; recorded run: {}",
            directory.display()
        );
        return Ok(Some(status));
    }
    collection?;
    eprintln!(
        "Collected training data; summary: {}",
        dataset.root.join("merged/summary.json").display()
    );
    Ok(Some(status))
}

fn compatible(previous: &BuildInvocation, current: &BuildInvocation) -> Result<()> {
    let semantic = |invocation: &BuildInvocation| -> Result<Value> {
        let mut value = serde_json::to_value(&invocation.options)?;
        let fields = value.as_object_mut().expect("build options are an object");
        for name in [
            "profile",
            "release",
            "profile_use",
            "profile_use_required",
            "debug_info",
            "target_dir",
            "cache_dir",
            "jobs",
            "message_format",
            "scoopc",
            "cc",
            "cxx",
            "linker",
            "unwind_prefix",
            "cone_path",
            "system_library_root",
            "runtime_root",
            "runtime_objects",
            "library_path",
        ] {
            fields.remove(name);
        }
        Ok(
            json!({"options": value, "gc_stress": invocation.enable_gc_stress, "startup_stats": invocation.enable_startup_stats}),
        )
    };
    if semantic(previous)? != semantic(current)? {
        return Err("this data directory belongs to a different root, target, export policy or semantic build configuration; use another --data-dir for that training configuration".into());
    }
    Ok(())
}

fn report_coverage(record: &Value) {
    for (field, message) in [
        ("uninstrumented_cones", "Precompiled Cones without counters"),
        ("missing_maps", "Missing profile maps"),
    ] {
        if let Some(values) = record["profiling"][field]
            .as_array()
            .filter(|values| !values.is_empty())
        {
            eprintln!("{message}: {}", Value::Array(values.clone()));
        }
    }
}
