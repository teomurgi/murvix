use murvix_protocol::{ErrorCode, ProjectDescriptor, ProjectPatch, ServerPath};

fn root(path: &str) -> ServerPath {
    ServerPath(path.as_bytes().to_vec())
}

#[test]
fn workspace_roots_reject_bad_paths_duplicates_and_overflow() {
    let valid = ProjectDescriptor {
        extra_directories: vec![root("/srv/api"), root("/srv/web")],
        ..ProjectDescriptor::new(root("/srv/app"), [])
    };
    assert_eq!(valid.validate(), Ok(()));
    assert_eq!(
        ProjectPatch::ExtraDirectories(valid.extra_directories.clone()).validate(),
        Ok(())
    );

    let primary = ProjectDescriptor {
        extra_directories: vec![root("/srv/app")],
        ..ProjectDescriptor::new(root("/srv/app"), [])
    };
    assert_eq!(primary.validate(), Err(ErrorCode::BadRequest));

    let relative = ProjectDescriptor {
        extra_directories: vec![root("srv/api")],
        ..ProjectDescriptor::new(root("/srv/app"), [])
    };
    assert_eq!(relative.validate(), Err(ErrorCode::BadRequest));

    let duplicate = ProjectDescriptor {
        extra_directories: vec![root("/srv/api"), root("/srv/api")],
        ..ProjectDescriptor::new(root("/srv/app"), [])
    };
    assert_eq!(duplicate.validate(), Err(ErrorCode::BadRequest));

    let many = ProjectDescriptor {
        extra_directories: (0..17)
            .map(|index| root(&format!("/srv/{index}")))
            .collect(),
        ..ProjectDescriptor::new(root("/srv/app"), [])
    };
    assert_eq!(many.validate(), Err(ErrorCode::BadRequest));
}
