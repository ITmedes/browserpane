use super::*;

type SecurityMutation = Box<dyn Fn(&mut ContainerSecurity)>;
type ResourceMutation = Box<dyn Fn(&mut ResourceLimits)>;

const IMAGE: &str =
    "registry.example/bpane-host@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn resource_id() -> Uuid {
    Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap()
}

fn limits() -> ResourceLimits {
    ResourceLimits {
        memory_bytes: 1_073_741_824,
        cpu_millis: 1_000,
        pids: 256,
        shm_bytes: 134_217_728,
        timeout_secs: 300,
        output_limit_bytes: 262_144,
    }
}

fn launch_policy() -> ContainerLaunchPolicy {
    ContainerLaunchPolicy {
        image: IMAGE.to_string(),
        container_name_prefix: "bpane-runtime".to_string(),
        network: Some("bpane-runtime-internal".to_string()),
        volume_prefixes: vec!["bpane-session-data".to_string()],
        fixed_volumes: BTreeSet::from(["bpane-runtime-socket".to_string()]),
        mount_targets: BTreeSet::from(["/run/bpane/session".to_string()]),
        require_read_only_mounts: false,
        environment_keys: BTreeSet::from([
            "BPANE_SESSION_ID".to_string(),
            "BPANE_AGENT_SOCKET".to_string(),
        ]),
        static_labels: BTreeMap::from([(
            "browserpane.runtime.managed".to_string(),
            "true".to_string(),
        )]),
        entrypoint: vec!["/usr/local/bin/start-host.sh".to_string()],
        added_capabilities: BTreeSet::new(),
        seccomp_profiles: BTreeSet::from(["browserpane-default".to_string()]),
        require_read_only_root_filesystem: false,
        maximum_resources: limits(),
    }
}

fn lifecycle_policy() -> LifecyclePolicy {
    LifecyclePolicy {
        container_name_prefix: "bpane-runtime".to_string(),
        volume_name_prefixes: vec!["bpane-session-data".to_string()],
        container_actions: BTreeSet::from([
            ContainerLifecycleAction::Inspect,
            ContainerLifecycleAction::Stop,
            ContainerLifecycleAction::Remove,
        ]),
        volume_actions: BTreeSet::from([VolumeLifecycleAction::Inspect]),
    }
}

fn broker_policy() -> RuntimeBrokerPolicy {
    RuntimeBrokerPolicy::new(ContainerPolicyConfig {
        launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, launch_policy())]),
        lifecycle: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, lifecycle_policy())]),
    })
    .unwrap()
}

fn expected_labels() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "browserpane.runtime.managed".to_string(),
            "true".to_string(),
        ),
        (
            "browserpane.runtime.operation".to_string(),
            "browser_runtime".to_string(),
        ),
        (
            "browserpane.runtime.resource_id".to_string(),
            resource_id().to_string(),
        ),
    ])
}

fn launch_spec() -> ContainerLaunchSpec {
    ContainerLaunchSpec {
        operation_kind: RuntimeOperationKind::BrowserRuntime,
        resource_id: resource_id(),
        owned_volume_ids: BTreeSet::from([resource_id()]),
        image: IMAGE.to_string(),
        container_name: format!("bpane-runtime-{}", resource_id().simple()),
        network: Some("bpane-runtime-internal".to_string()),
        mounts: vec![ContainerMount {
            source: MountSource::NamedVolume(format!(
                "bpane-session-data-{}",
                resource_id().simple()
            )),
            target: "/run/bpane/session".to_string(),
            read_only: false,
        }],
        environment_keys: BTreeSet::from([
            "BPANE_SESSION_ID".to_string(),
            "BPANE_AGENT_SOCKET".to_string(),
        ]),
        labels: expected_labels(),
        entrypoint: vec!["/usr/local/bin/start-host.sh".to_string()],
        security: ContainerSecurity {
            privileged: false,
            host_network: false,
            host_pid: false,
            host_ipc: false,
            devices: Vec::new(),
            added_capabilities: BTreeSet::new(),
            no_new_privileges: true,
            read_only_root_filesystem: false,
            seccomp_profile: "browserpane-default".to_string(),
        },
        resources: limits(),
    }
}

fn assert_launch_denied(spec: &ContainerLaunchSpec, code: PolicyErrorCode) {
    assert_eq!(
        broker_policy().authorize_launch(spec),
        Err(PolicyViolation { code })
    );
}

#[test]
fn accepts_exact_owned_launch_specification() {
    broker_policy().authorize_launch(&launch_spec()).unwrap();
}

#[test]
fn denies_unknown_operation_family() {
    let mut spec = launch_spec();
    spec.operation_kind = RuntimeOperationKind::WorkflowWorker;

    assert_launch_denied(&spec, PolicyErrorCode::OperationNotAllowed);
}

#[test]
fn denies_unapproved_image_name_and_network() {
    let mut spec = launch_spec();
    spec.image = "attacker/image:latest".to_string();
    assert_launch_denied(&spec, PolicyErrorCode::ImageNotAllowed);

    let mut spec = launch_spec();
    spec.container_name = "unrelated-container".to_string();
    assert_launch_denied(&spec, PolicyErrorCode::ContainerNameNotOwned);

    let mut spec = launch_spec();
    spec.network = Some("host".to_string());
    assert_launch_denied(&spec, PolicyErrorCode::NetworkNotAllowed);
}

#[test]
fn denies_host_mounts_unowned_volumes_targets_and_duplicates() {
    let mut spec = launch_spec();
    spec.mounts[0].source = MountSource::HostPath("/".to_string());
    assert_launch_denied(&spec, PolicyErrorCode::MountNotAllowed);

    let mut spec = launch_spec();
    spec.mounts[0].source = MountSource::NamedVolume("unrelated-volume".to_string());
    assert_launch_denied(&spec, PolicyErrorCode::MountNotAllowed);

    let mut spec = launch_spec();
    spec.mounts[0].source = MountSource::NamedVolume("bpane-session-database-escape".to_string());
    assert_launch_denied(&spec, PolicyErrorCode::MountNotAllowed);

    let mut spec = launch_spec();
    spec.mounts[0].target = "/host".to_string();
    assert_launch_denied(&spec, PolicyErrorCode::MountNotAllowed);

    let mut spec = launch_spec();
    spec.mounts.push(spec.mounts[0].clone());
    assert_launch_denied(&spec, PolicyErrorCode::MountNotAllowed);
}

#[test]
fn enforces_read_only_mount_policy_when_selected() {
    let mut policy = launch_policy();
    policy.require_read_only_mounts = true;
    let evaluator = RuntimeBrokerPolicy::new(ContainerPolicyConfig {
        launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, policy)]),
        lifecycle: BTreeMap::new(),
    })
    .unwrap();

    assert_eq!(
        evaluator.authorize_launch(&launch_spec()),
        Err(PolicyErrorCode::MountNotAllowed.into())
    );
}

#[test]
fn denies_unknown_environment_keys_and_label_changes() {
    let mut spec = launch_spec();
    spec.environment_keys.insert("LD_PRELOAD".to_string());
    assert_launch_denied(&spec, PolicyErrorCode::EnvironmentNotAllowed);

    let mut spec = launch_spec();
    spec.labels
        .insert("attacker.label".to_string(), "value".to_string());
    assert_launch_denied(&spec, PolicyErrorCode::LabelsInvalid);

    let mut spec = launch_spec();
    spec.labels.remove("browserpane.runtime.resource_id");
    assert_launch_denied(&spec, PolicyErrorCode::LabelsInvalid);
}

#[test]
fn denies_entrypoint_and_security_escalation() {
    let mut spec = launch_spec();
    spec.entrypoint = vec!["/bin/sh".to_string()];
    assert_launch_denied(&spec, PolicyErrorCode::EntrypointNotAllowed);

    let mut candidates: Vec<SecurityMutation> = vec![
        Box::new(|security| security.privileged = true),
        Box::new(|security| security.host_network = true),
        Box::new(|security| security.host_pid = true),
        Box::new(|security| security.host_ipc = true),
        Box::new(|security| security.devices.push("/dev/kvm".to_string())),
        Box::new(|security| {
            security.added_capabilities.insert("SYS_ADMIN".to_string());
        }),
        Box::new(|security| security.no_new_privileges = false),
        Box::new(|security| security.seccomp_profile = "unconfined".to_string()),
    ];
    for mutate in candidates.drain(..) {
        let mut spec = launch_spec();
        mutate(&mut spec.security);
        assert_launch_denied(&spec, PolicyErrorCode::SecuritySettingsNotAllowed);
    }
}

#[test]
fn enforces_read_only_root_filesystem_when_selected() {
    let mut policy = launch_policy();
    policy.require_read_only_root_filesystem = true;
    let evaluator = RuntimeBrokerPolicy::new(ContainerPolicyConfig {
        launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, policy)]),
        lifecycle: BTreeMap::new(),
    })
    .unwrap();

    assert_eq!(
        evaluator.authorize_launch(&launch_spec()),
        Err(PolicyErrorCode::SecuritySettingsNotAllowed.into())
    );
}

#[test]
fn denies_zero_or_excessive_resource_limits() {
    let mutations: Vec<ResourceMutation> = vec![
        Box::new(|limits| limits.memory_bytes = 0),
        Box::new(|limits| limits.memory_bytes += 1),
        Box::new(|limits| limits.cpu_millis += 1),
        Box::new(|limits| limits.pids += 1),
        Box::new(|limits| limits.shm_bytes += 1),
        Box::new(|limits| limits.timeout_secs += 1),
        Box::new(|limits| limits.output_limit_bytes += 1),
    ];
    for mutate in mutations {
        let mut spec = launch_spec();
        mutate(&mut spec.resources);
        assert_launch_denied(&spec, PolicyErrorCode::ResourceLimitsExceeded);
    }
}

#[test]
fn validates_owned_container_lifecycle_target_and_action() {
    let target = OwnedContainerTarget {
        operation_kind: RuntimeOperationKind::BrowserRuntime,
        resource_id: resource_id(),
        container_name: format!("bpane-runtime-{}", resource_id().simple()),
        action: ContainerLifecycleAction::Stop,
    };
    broker_policy()
        .authorize_container_lifecycle(&target)
        .unwrap();

    let mut unowned = target.clone();
    unowned.container_name = "unrelated-container".to_string();
    assert_eq!(
        broker_policy().authorize_container_lifecycle(&unowned),
        Err(PolicyErrorCode::LifecycleTargetNotOwned.into())
    );

    let mut config = ContainerPolicyConfig {
        launch: BTreeMap::new(),
        lifecycle: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, lifecycle_policy())]),
    };
    config
        .lifecycle
        .get_mut(&RuntimeOperationKind::BrowserRuntime)
        .unwrap()
        .container_actions
        .remove(&ContainerLifecycleAction::Stop);
    assert_eq!(
        RuntimeBrokerPolicy::new(config)
            .unwrap()
            .authorize_container_lifecycle(&target),
        Err(PolicyErrorCode::LifecycleActionNotAllowed.into())
    );
}

#[test]
fn validates_owned_volume_lifecycle_target_and_action() {
    let target = OwnedVolumeTarget {
        operation_kind: RuntimeOperationKind::BrowserRuntime,
        resource_id: resource_id(),
        volume_name: format!("bpane-session-data-{}", resource_id().simple()),
        action: VolumeLifecycleAction::Inspect,
    };
    broker_policy().authorize_volume_lifecycle(&target).unwrap();

    let mut unowned = target.clone();
    unowned.volume_name = "unrelated-volume".to_string();
    assert_eq!(
        broker_policy().authorize_volume_lifecycle(&unowned),
        Err(PolicyErrorCode::LifecycleTargetNotOwned.into())
    );

    let mut disallowed = target;
    disallowed.action = VolumeLifecycleAction::Remove;
    assert_eq!(
        broker_policy().authorize_volume_lifecycle(&disallowed),
        Err(PolicyErrorCode::LifecycleActionNotAllowed.into())
    );
}

#[test]
fn policy_errors_do_not_echo_submitted_values() {
    let submitted = "/private/host/path";
    let mut spec = launch_spec();
    spec.mounts[0].source = MountSource::HostPath(submitted.to_string());

    let error = broker_policy().authorize_launch(&spec).unwrap_err();
    let message = error.to_string();

    assert!(!message.contains(submitted));
    assert_eq!(error.code, PolicyErrorCode::MountNotAllowed);
}

#[test]
fn accepts_fixed_shared_volume_and_rejects_other_resource_volume() {
    let mut fixed = launch_spec();
    fixed.mounts[0].source = MountSource::NamedVolume("bpane-runtime-socket".to_string());
    broker_policy().authorize_launch(&fixed).unwrap();

    let mut other = launch_spec();
    other.mounts[0].source =
        MountSource::NamedVolume(format!("bpane-session-data-{}", Uuid::now_v7().simple()));
    assert_launch_denied(&other, PolicyErrorCode::MountNotAllowed);

    other.owned_volume_ids.insert(Uuid::nil());
    assert_launch_denied(&other, PolicyErrorCode::MountNotAllowed);
}

#[test]
fn validates_trusted_policy_configuration() {
    let valid = ContainerPolicyConfig {
        launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, launch_policy())]),
        lifecycle: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, lifecycle_policy())]),
    };
    RuntimeBrokerPolicy::new(valid).unwrap();

    let mut mutable_image = launch_policy();
    mutable_image.image = "registry.example/bpane-host:latest".to_string();
    assert_eq!(
        RuntimeBrokerPolicy::new(ContainerPolicyConfig {
            launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, mutable_image)]),
            lifecycle: BTreeMap::new(),
        })
        .unwrap_err(),
        PolicyConfigurationErrorCode::InvalidImage.into()
    );

    let mut host_network = launch_policy();
    host_network.network = Some("host".to_string());
    assert_eq!(
        RuntimeBrokerPolicy::new(ContainerPolicyConfig {
            launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, host_network)]),
            lifecycle: BTreeMap::new(),
        })
        .unwrap_err(),
        PolicyConfigurationErrorCode::InvalidNetwork.into()
    );
}

#[test]
fn rejects_reserved_labels_unsafe_paths_and_unbounded_limits() {
    let cases = [
        (
            {
                let mut policy = launch_policy();
                policy.container_name_prefix = "/unsafe".to_string();
                policy
            },
            PolicyConfigurationErrorCode::InvalidContainerNamePrefix,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.fixed_volumes.insert("/host/path".to_string());
                policy
            },
            PolicyConfigurationErrorCode::InvalidVolumePolicy,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.static_labels.insert(
                    "browserpane.runtime.resource_id".to_string(),
                    "override".to_string(),
                );
                policy
            },
            PolicyConfigurationErrorCode::InvalidLabels,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.mount_targets.insert("/run/../host".to_string());
                policy
            },
            PolicyConfigurationErrorCode::InvalidMountTarget,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.environment_keys.insert("lowercase".to_string());
                policy
            },
            PolicyConfigurationErrorCode::InvalidEnvironmentKey,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.entrypoint.clear();
                policy
            },
            PolicyConfigurationErrorCode::InvalidEntrypoint,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.seccomp_profiles = BTreeSet::from(["unconfined".to_string()]);
                policy
            },
            PolicyConfigurationErrorCode::InvalidSeccompProfile,
        ),
        (
            {
                let mut policy = launch_policy();
                policy.maximum_resources.memory_bytes = 0;
                policy
            },
            PolicyConfigurationErrorCode::InvalidResourceLimits,
        ),
    ];

    for (policy, code) in cases {
        assert_eq!(
            RuntimeBrokerPolicy::new(ContainerPolicyConfig {
                launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, policy)]),
                lifecycle: BTreeMap::new(),
            })
            .unwrap_err(),
            code.into()
        );
    }
}

#[test]
fn rejects_incomplete_or_inconsistent_lifecycle_policy() {
    let empty = LifecyclePolicy {
        container_name_prefix: "bpane-runtime".to_string(),
        volume_name_prefixes: Vec::new(),
        container_actions: BTreeSet::new(),
        volume_actions: BTreeSet::new(),
    };
    assert_eq!(
        RuntimeBrokerPolicy::new(ContainerPolicyConfig {
            launch: BTreeMap::new(),
            lifecycle: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, empty)]),
        })
        .unwrap_err(),
        PolicyConfigurationErrorCode::InvalidLifecyclePolicy.into()
    );

    let mut mismatched = lifecycle_policy();
    mismatched.container_name_prefix = "different-runtime".to_string();
    assert_eq!(
        RuntimeBrokerPolicy::new(ContainerPolicyConfig {
            launch: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, launch_policy())]),
            lifecycle: BTreeMap::from([(RuntimeOperationKind::BrowserRuntime, mismatched)]),
        })
        .unwrap_err(),
        PolicyConfigurationErrorCode::InvalidLifecyclePolicy.into()
    );
}
