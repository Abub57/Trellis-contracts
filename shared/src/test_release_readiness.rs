use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, Symbol, Vec};

use crate::migration::RollbackStrategy;
use crate::release_readiness::{
    create_gate_check, evaluate_release_readiness, get_readiness_report, get_rehearsal_count,
    get_rehearsal_receipt, is_release_ready, rehearse_rollback, GateCheckStatus, GateKind,
    ReleaseGateError, ReleaseManifest, ReleaseModule, RollbackRehearsalParams,
};

#[test]
fn test_release_readiness_gate_all_modules_passing() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Treasury);
    modules.push_back(ReleaseModule::Aid);
    modules.push_back(ReleaseModule::Referral);
    modules.push_back(ReleaseModule::Governance);
    modules.push_back(ReleaseModule::Oracle);
    modules.push_back(ReleaseModule::Marketplace);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    for i in 0..modules.len() {
        let module = modules.get(i).unwrap();

        // 1. Migration Gate
        checks.push_back(create_gate_check(
            symbol_short!("mig_v2"),
            GateKind::Migration,
            module,
            GateCheckStatus::Passed,
            symbol_short!("mig_ok"),
            now,
        ));

        // 2. Security Gate
        checks.push_back(create_gate_check(
            symbol_short!("sec_aud"),
            GateKind::Security,
            module,
            GateCheckStatus::Passed,
            symbol_short!("aud_ok"),
            now,
        ));

        // 3. Smoke Test Gate
        checks.push_back(create_gate_check(
            symbol_short!("smk_wir"),
            GateKind::SmokeTest,
            module,
            GateCheckStatus::Passed,
            symbol_short!("smk_ok"),
            now,
        ));

        // 4. Rollback Rehearsal Gate
        checks.push_back(create_gate_check(
            symbol_short!("rlb_reh"),
            GateKind::RollbackRehearsal,
            module,
            GateCheckStatus::Passed,
            symbol_short!("rlb_ok"),
            now,
        ));
    }

    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_2_0"),
        modules,
        checks,
        signoff_admin: admin,
        exception_marker: None,
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(report.is_ready);
    assert_eq!(report.passed_checks, 24);
    assert_eq!(report.failed_checks, 0);
    assert_eq!(report.missing_gates, 0);
    assert_eq!(report.bypassed_checks, 0);

    assert!(is_release_ready(&env, &symbol_short!("rel_2_0")));
    let retrieved = get_readiness_report(&env, &symbol_short!("rel_2_0")).unwrap();
    assert_eq!(retrieved.release_version, symbol_short!("rel_2_0"));
    assert!(retrieved.is_ready);
}

#[test]
fn test_blocks_release_when_migration_missing() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Aid);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    // Missing GateKind::Migration!
    checks.push_back(create_gate_check(
        symbol_short!("sec_aud"),
        GateKind::Security,
        ReleaseModule::Aid,
        GateCheckStatus::Passed,
        symbol_short!("aud_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("smk_wir"),
        GateKind::SmokeTest,
        ReleaseModule::Aid,
        GateCheckStatus::Passed,
        symbol_short!("smk_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("rlb_reh"),
        GateKind::RollbackRehearsal,
        ReleaseModule::Aid,
        GateCheckStatus::Passed,
        symbol_short!("rlb_ok"),
        now,
    ));

    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_aid"),
        modules,
        checks,
        signoff_admin: admin,
        exception_marker: None,
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(!report.is_ready);
    assert_eq!(report.missing_gates, 1);
    assert!(!is_release_ready(&env, &symbol_short!("rel_aid")));
}

#[test]
fn test_blocks_release_when_security_check_missing() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Treasury);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    checks.push_back(create_gate_check(
        symbol_short!("mig_v2"),
        GateKind::Migration,
        ReleaseModule::Treasury,
        GateCheckStatus::Passed,
        symbol_short!("mig_ok"),
        now,
    ));
    // Missing GateKind::Security!
    checks.push_back(create_gate_check(
        symbol_short!("smk_wir"),
        GateKind::SmokeTest,
        ReleaseModule::Treasury,
        GateCheckStatus::Passed,
        symbol_short!("smk_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("rlb_reh"),
        GateKind::RollbackRehearsal,
        ReleaseModule::Treasury,
        GateCheckStatus::Passed,
        symbol_short!("rlb_ok"),
        now,
    ));

    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_trs"),
        modules,
        checks,
        signoff_admin: admin,
        exception_marker: None,
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(!report.is_ready);
    assert_eq!(report.missing_gates, 1);
}

#[test]
fn test_blocks_release_when_smoke_test_missing() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Marketplace);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    checks.push_back(create_gate_check(
        symbol_short!("mig_v2"),
        GateKind::Migration,
        ReleaseModule::Marketplace,
        GateCheckStatus::Passed,
        symbol_short!("mig_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("sec_aud"),
        GateKind::Security,
        ReleaseModule::Marketplace,
        GateCheckStatus::Passed,
        symbol_short!("aud_ok"),
        now,
    ));
    // Missing GateKind::SmokeTest!
    checks.push_back(create_gate_check(
        symbol_short!("rlb_reh"),
        GateKind::RollbackRehearsal,
        ReleaseModule::Marketplace,
        GateCheckStatus::Passed,
        symbol_short!("rlb_ok"),
        now,
    ));

    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_mkt"),
        modules,
        checks,
        signoff_admin: admin,
        exception_marker: None,
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(!report.is_ready);
    assert_eq!(report.missing_gates, 1);
}

#[test]
fn test_blocks_release_when_rollback_verification_missing() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Oracle);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    checks.push_back(create_gate_check(
        symbol_short!("mig_v2"),
        GateKind::Migration,
        ReleaseModule::Oracle,
        GateCheckStatus::Passed,
        symbol_short!("mig_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("sec_aud"),
        GateKind::Security,
        ReleaseModule::Oracle,
        GateCheckStatus::Passed,
        symbol_short!("aud_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("smk_wir"),
        GateKind::SmokeTest,
        ReleaseModule::Oracle,
        GateCheckStatus::Passed,
        symbol_short!("smk_ok"),
        now,
    ));
    // Missing GateKind::RollbackRehearsal!

    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_orc"),
        modules,
        checks,
        signoff_admin: admin,
        exception_marker: None,
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(!report.is_ready);
    assert_eq!(report.missing_gates, 1);
}

#[test]
fn test_blocks_release_on_failed_or_pending_check() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Referral);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    checks.push_back(create_gate_check(
        symbol_short!("mig_v2"),
        GateKind::Migration,
        ReleaseModule::Referral,
        GateCheckStatus::Failed, // Check failed!
        symbol_short!("err_mig"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("sec_aud"),
        GateKind::Security,
        ReleaseModule::Referral,
        GateCheckStatus::Passed,
        symbol_short!("aud_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("smk_wir"),
        GateKind::SmokeTest,
        ReleaseModule::Referral,
        GateCheckStatus::Passed,
        symbol_short!("smk_ok"),
        now,
    ));
    checks.push_back(create_gate_check(
        symbol_short!("rlb_reh"),
        GateKind::RollbackRehearsal,
        ReleaseModule::Referral,
        GateCheckStatus::Passed,
        symbol_short!("rlb_ok"),
        now,
    ));

    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_ref"),
        modules,
        checks,
        signoff_admin: admin,
        exception_marker: None,
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(!report.is_ready);
    assert_eq!(report.failed_checks, 1);
}

#[test]
fn test_emergency_exception_marker_bypass() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let mut modules = Vec::new(&env);
    modules.push_back(ReleaseModule::Governance);

    let mut checks = Vec::new(&env);
    let now = 1_000_000u64;

    // Has exception marker
    let manifest = ReleaseManifest {
        release_version: symbol_short!("rel_emrg"),
        modules,
        checks, // empty checks bypassed by emergency exception
        signoff_admin: admin,
        exception_marker: Some(symbol_short!("emrg_fix")),
    };

    let report = evaluate_release_readiness(&env, &manifest).unwrap();
    assert!(report.is_ready);
    assert_eq!(report.missing_gates, 0);
    assert_eq!(report.bypassed_checks, 4);
    assert!(is_release_ready(&env, &symbol_short!("rel_emrg")));
}

#[test]
fn test_rollback_rehearsal_revert_strategy() {
    let env = Env::default();
    let prev_hash = BytesN::from_array(&env, &[7u8; 32]);

    let params = RollbackRehearsalParams {
        module: ReleaseModule::Aid,
        current_version: 1,
        target_version: 2,
        previous_wasm_hash: prev_hash,
        strategy: RollbackStrategy::Revert,
        simulate_regression: true,
    };

    let receipt = rehearse_rollback(&env, &params).unwrap();
    assert_eq!(receipt.rehearsal_id, 1);
    assert_eq!(receipt.module, ReleaseModule::Aid);
    assert_eq!(receipt.from_version, 2);
    assert_eq!(receipt.restored_version, 1);
    assert!(receipt.state_intact);
    assert!(receipt.passed);

    assert_eq!(get_rehearsal_count(&env), 1);
    let fetched = get_rehearsal_receipt(&env, 1).unwrap();
    assert_eq!(fetched.rehearsal_id, 1);
    assert!(fetched.passed);
}

#[test]
fn test_rollback_rehearsal_forward_fix_strategy() {
    let env = Env::default();
    let prev_hash = BytesN::from_array(&env, &[8u8; 32]);

    let params = RollbackRehearsalParams {
        module: ReleaseModule::Treasury,
        current_version: 1,
        target_version: 2,
        previous_wasm_hash: prev_hash,
        strategy: RollbackStrategy::ForwardFix,
        simulate_regression: true,
    };

    let receipt = rehearse_rollback(&env, &params).unwrap();
    assert_eq!(receipt.restored_version, 2);
    assert!(receipt.state_intact);
    assert!(receipt.passed);
}

#[test]
fn test_rollback_rehearsal_manual_strategy_blocks() {
    let env = Env::default();
    let prev_hash = BytesN::from_array(&env, &[9u8; 32]);

    let params = RollbackRehearsalParams {
        module: ReleaseModule::Governance,
        current_version: 1,
        target_version: 2,
        previous_wasm_hash: prev_hash,
        strategy: RollbackStrategy::Manual,
        simulate_regression: true,
    };

    let receipt = rehearse_rollback(&env, &params).unwrap();
    // Manual strategy cannot automatically recover without operator action!
    assert!(!receipt.passed);
    assert!(!receipt.state_intact);
}

#[test]
fn test_rehearsal_rejects_invalid_version_order() {
    let env = Env::default();
    let prev_hash = BytesN::from_array(&env, &[1u8; 32]);

    let params = RollbackRehearsalParams {
        module: ReleaseModule::Marketplace,
        current_version: 2,
        target_version: 2, // Must be > current_version
        previous_wasm_hash: prev_hash,
        strategy: RollbackStrategy::Revert,
        simulate_regression: false,
    };

    let err = rehearse_rollback(&env, &params).unwrap_err();
    assert_eq!(err, ReleaseGateError::RehearsalSimulationFailed);
}
