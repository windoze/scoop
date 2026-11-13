use super::*;
use object::read::macho::MachOFile64;
use scoop_slib::MachOArchitecture;

pub(super) fn append_libraries(
    profile: &DarwinFinalLinkProfile,
    command: &mut Command,
    directory: &Path,
) {
    let machine = MachOArchitecture::for_target(profile.id()).unwrap();
    let system = profile.system_provider();
    let stub = directory.join("libSystem.tbd");
    std::fs::write(
        &stub,
        crate::write_link_stub(
            machine,
            system.install_name(),
            system.current_version(),
            system.compatibility_version(),
            system.exports(),
        )
        .unwrap(),
    )
    .unwrap();
    command.arg(stub);
    if !profile.cxx() {
        return;
    }
    let startup = profile.startup_toolchain();
    let bytes = std::fs::read(startup.sdk_root().unwrap().join("usr/lib/libc++.tbd")).unwrap();
    let deployment = startup
        .profile()
        .contract()
        .deployment()
        .unwrap()
        .minimum_os();
    for (index, interface) in crate::read_text_stubs(machine, &bytes, deployment, true)
        .unwrap()
        .into_iter()
        .enumerate()
    {
        let stub = directory.join(format!("libcxx-{index}.tbd"));
        std::fs::write(
            &stub,
            crate::write_link_stub(
                machine,
                &interface.install_name,
                interface.current_version,
                interface.compatibility_version,
                &interface.exports,
            )
            .unwrap(),
        )
        .unwrap();
        command.arg(stub);
    }
}

pub(super) fn assert_lld(path: &Path) {
    let bytes = std::fs::read(path).unwrap();
    let macho = MachOFile64::<object::Endianness>::parse(bytes.as_slice()).unwrap();
    let endian = macho.endian();
    let mut commands = macho.macho_load_commands().unwrap();
    let mut tools = Vec::new();
    while let Some(command) = commands.next().unwrap() {
        if let Some(build) = command.build_version().unwrap() {
            tools.extend(
                command.raw_data()[24..]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .take(build.ntools.get(endian) as usize)
                    .map(|tool| u32::from_le_bytes(tool[..4].try_into().unwrap())),
            );
        }
    }
    assert_eq!(tools, [4], "LC_BUILD_VERSION must record TOOL_LLD");
    assert!(macho.section_by_name("__unwind_info").unwrap().size() > 0);
}
