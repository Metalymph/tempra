use tempra_ast::{
    SpecializedCategory, SpecializedDenyTarget, SpecializedFamily, SpecializedOperation,
    SpecializedParameter,
};

#[test]
fn specialized_operation_names_are_closed_and_canonical() {
    let expected = [
        ("fs.read", SpecializedOperation::FsRead),
        ("fs.write", SpecializedOperation::FsWrite),
        ("fs.remove", SpecializedOperation::FsRemove),
        ("net.connect", SpecializedOperation::NetConnect),
        ("net.listen", SpecializedOperation::NetListen),
        ("net.send", SpecializedOperation::NetSend),
        ("net.receive", SpecializedOperation::NetReceive),
        ("io.read", SpecializedOperation::IoRead),
        ("io.write", SpecializedOperation::IoWrite),
        ("process.exec", SpecializedOperation::ProcessExec),
        ("process.exit", SpecializedOperation::ProcessExit),
        ("spawn.task", SpecializedOperation::SpawnTask),
        ("spawn.actor", SpecializedOperation::SpawnActor),
    ];
    assert_eq!(
        SpecializedOperation::ALL
            .iter()
            .map(|operation| operation.canonical_name())
            .collect::<Vec<_>>(),
        expected.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for (name, operation) in expected {
        assert_eq!(SpecializedOperation::from_name(name), Some(operation));
    }
    for alias in [
        "fs.delete",
        "fs.read_text",
        "net.open",
        "io.console",
        "process.spawn",
        "spawn.thread",
        "FS.READ",
    ] {
        assert_eq!(SpecializedOperation::from_name(alias), None, "{alias}");
    }
}

#[test]
fn specialized_families_and_categories_are_closed_and_canonical() {
    assert_eq!(
        SpecializedFamily::ALL,
        [SpecializedFamily::Fs, SpecializedFamily::Net]
    );
    assert_eq!(
        SpecializedFamily::from_name("fs"),
        Some(SpecializedFamily::Fs)
    );
    assert_eq!(
        SpecializedFamily::from_name("net"),
        Some(SpecializedFamily::Net)
    );
    assert_eq!(SpecializedFamily::from_name("io"), None);
    assert_eq!(SpecializedFamily::Fs.canonical_name(), "fs");
    assert_eq!(SpecializedFamily::Net.canonical_name(), "net");

    assert_eq!(
        SpecializedCategory::ALL,
        [SpecializedCategory::IoRead, SpecializedCategory::IoWrite]
    );
    assert_eq!(
        SpecializedCategory::from_name("io.read"),
        Some(SpecializedCategory::IoRead)
    );
    assert_eq!(
        SpecializedCategory::from_name("io.write"),
        Some(SpecializedCategory::IoWrite)
    );
    assert_eq!(SpecializedCategory::from_name("io"), None);
    assert_eq!(SpecializedCategory::IoRead.canonical_name(), "io.read");
    assert_eq!(SpecializedCategory::IoWrite.canonical_name(), "io.write");
}

#[test]
fn deny_targets_keep_family_category_and_operation_distinct() {
    let family = SpecializedDenyTarget::Family(SpecializedFamily::Net);
    assert_eq!(family.canonical_name(), "net");
    assert_eq!(family.operation(), None);

    let category = SpecializedDenyTarget::Category(SpecializedCategory::IoRead);
    assert_eq!(category.canonical_name(), "io.read");
    assert_eq!(category.operation(), None);

    let operation = SpecializedDenyTarget::Operation(SpecializedOperation::NetReceive);
    assert_eq!(operation.canonical_name(), "net.receive");
    assert_eq!(
        operation.operation(),
        Some(SpecializedOperation::NetReceive)
    );
}

#[test]
fn specialized_parameter_keys_match_the_closed_vocabulary() {
    let expected = [
        ("source", SpecializedParameter::Source),
        ("destination", SpecializedParameter::Destination),
        ("host", SpecializedParameter::Host),
        ("port", SpecializedParameter::Port),
        ("protocol", SpecializedParameter::Protocol),
        ("executable", SpecializedParameter::Executable),
        ("stream", SpecializedParameter::Stream),
        ("scope", SpecializedParameter::Scope),
    ];
    assert_eq!(
        SpecializedParameter::ALL
            .iter()
            .map(|parameter| parameter.canonical_name())
            .collect::<Vec<_>>(),
        expected.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for (name, parameter) in expected {
        assert_eq!(SpecializedParameter::from_name(name), Some(parameter));
    }
    for alias in ["path", "Destination", "file", "url"] {
        assert_eq!(SpecializedParameter::from_name(alias), None, "{alias}");
    }
}

#[test]
fn operation_parameter_membership_matches_the_taxonomy() {
    use SpecializedParameter::*;
    assert_eq!(
        SpecializedOperation::FsRead.valid_parameter_keys(),
        &[Source][..]
    );
    assert_eq!(
        SpecializedOperation::FsWrite.valid_parameter_keys(),
        &[Destination][..]
    );
    assert_eq!(
        SpecializedOperation::NetListen.valid_parameter_keys(),
        &[Host, Port, Protocol][..]
    );
    assert_eq!(
        SpecializedOperation::IoWrite.valid_parameter_keys(),
        &[Stream][..]
    );
    assert!(SpecializedOperation::ProcessExec.accepts_parameter(Executable));
    assert!(!SpecializedOperation::ProcessExec.accepts_parameter(Source));
    assert!(SpecializedOperation::NetSend
        .valid_parameter_keys()
        .is_empty());
    assert!(SpecializedOperation::NetReceive
        .valid_parameter_keys()
        .is_empty());
    assert!(SpecializedOperation::ProcessExit
        .valid_parameter_keys()
        .is_empty());
    assert!(SpecializedOperation::SpawnTask.accepts_parameter(Scope));
}
