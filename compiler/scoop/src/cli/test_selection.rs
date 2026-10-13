use scoop_slib::TestDirectory;

#[cfg(test)]
mod tests;
use std::ffi::OsString;

pub(super) struct SelectedTests {
    pub directory: TestDirectory,
    pub arguments: Vec<OsString>,
    pub empty_reason: Option<&'static str>,
}

pub(super) fn select(
    directory: &TestDirectory,
    filter: Option<&str>,
    ignored: bool,
) -> SelectedTests {
    let mut arguments = vec![OsString::from("--scoop-test-cases")];
    let mut selected = TestDirectory::default();
    let mut index = 0usize;
    let mut matching = 0usize;
    for container in &directory.containers {
        let mut copy = container.clone();
        copy.cases.retain(|case| {
            let current = index;
            index += 1;
            let matches = filter.is_none_or(|filter| case.display_name.contains(filter));
            matching += usize::from(matches);
            let keep = matches && case.ignore_reason.is_some() == ignored;
            if keep {
                arguments.push(current.to_string().into());
            }
            keep
        });
        if !copy.cases.is_empty() {
            selected.containers.push(copy);
        }
    }
    let empty_reason = if !selected.containers.is_empty() {
        None
    } else if index == 0 {
        Some("no tests")
    } else if matching == 0 {
        Some("no matching tests")
    } else if ignored {
        Some("no ignored tests")
    } else {
        Some("all matching tests ignored")
    };
    SelectedTests {
        directory: selected,
        arguments,
        empty_reason,
    }
}
