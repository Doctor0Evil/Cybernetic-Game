// path: src/cybergen/regulator/device_penalties.rs

use crate::cybergen::regulator::{
    CyberneticPlayer,
    RiskSample,
    consent_hash,
    consent_hash_mod_u64,
    cybernetic_energy_drain,
};
use core::cmp::max;

/// Device class for augmentation hardware. [file:1]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Implant,          // fully implanted, cybernetic-chipset, biodegradable, organic-compatible [file:1]
    SoftwareOnly,     // pure software / virtual deployment, no external physical shell [file:1]
    Wearable,         // non-implanted wearable, bands, goggles, exosleeves [file:1]
    Peripheral,       // desktop rigs, handheld controllers, non-body-mounted [file:1]
}

/// Difficulty and consequence multipliers based on device class. [file:1]
#[derive(Debug, Clone, Copy)]
pub struct DevicePenaltyProfile {
    pub df_multiplier: f64,    // multiplies D_f (compliance difficulty) [file:1]
    pub psych_multiplier: f64, // multiplies S_f (psych risk) [file:1]
    pub energy_multiplier: f64,// multiplies E_d (energy drain) [file:1]
    pub hard_lockout: bool,    // true = certain challenges cannot be satisfied [file:1]
}

/// Static profiles enforcing harsher penalties for non-implant hardware. [file:1]
pub fn device_penalty_profile(class: DeviceClass) -> DevicePenaltyProfile {
    match class {
        DeviceClass::Implant => DevicePenaltyProfile {
            df_multiplier: 1.0,
            psych_multiplier: 1.0,
            energy_multiplier: 1.0,
            hard_lockout: false,
        },
        DeviceClass::SoftwareOnly => DevicePenaltyProfile {
            df_multiplier: 1.25,
            psych_multiplier: 1.3,
            energy_multiplier: 1.2,
            hard_lockout: false,
        },
        DeviceClass::Wearable => DevicePenaltyProfile {
            df_multiplier: 1.6,
            psych_multiplier: 1.8,
            energy_multiplier: 1.7,
            hard_lockout: true,
        },
        DeviceClass::Peripheral => DevicePenaltyProfile {
            df_multiplier: 2.0,
            psych_multiplier: 2.2,
            energy_multiplier: 2.0,
            hard_lockout: true,
        },
    }
}

/// Compliance penetration depth → difficulty increment:
/// D_f' = D_f * (1 + 0.1 * C_p), where C_p = 100 * depth, already handled by base regulator. [file:1]
///
/// This function injects a +0.1 score for every 0.01 depth by increasing effective depth before
/// calling the core D_f logic (depth' = depth + 0.01 * floor(depth / 0.01)). [file:1]
pub fn boosted_depth_for_penetration(depth: f64) -> f64 {
    if depth <= 0.0 {
        return 0.0;
    }
    let steps = (depth / 0.01).floor();              // number of 0.01 penetration slices [file:1]
    let bonus = 0.01 * steps;                        // 0.01 depth bonus per slice [file:1]
    depth + bonus
}

/// Game-plane challenge classification for an upgrade attempt. [file:1]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeClass {
    Feasible,          // can be attempted under current hardware and safety envelope [file:1]
    HardMode,          // feasible but with heavy multipliers and risk [file:1]
    Incompatible,      // structurally impossible with this hardware class [file:1]
}

/// Reason codes when marking a challenge as incompatible. [file:1]
#[derive(Debug, Clone)]
pub struct Incompatibility {
    pub reason_code: u32,
    pub message: &'static str,
}

/// Combined evaluation result for a single upgrade or research action. [file:1]
#[derive(Debug, Clone)]
pub struct UpgradeEvaluation {
    pub challenge_class: ChallengeClass,
    pub device_class: DeviceClass,
    pub penalties: DevicePenaltyProfile,
    pub risk_sample: Option<RiskSample>,
    pub incompatibility: Option<Incompatibility>,
}

/// Determine if the requested upgrade target intrinsically requires implanted hardware. [file:1]
pub fn requires_implant_only(target: &str) -> bool {
    let t = target.to_ascii_lowercase();
    t.contains("deep-neuro") ||
    t.contains("spike-train") ||
    t.contains("blood-drain") ||
    t.contains("direct-bci") ||
    t.contains("qpu-soma-bridge")
}

/// Evaluate an upgrade request against device class and cybernetic penalties. [file:1]
///
/// - Enforces automatic-consent via consent_hash.
/// - Applies device-specific multipliers to depth, psych-risk, and energy-drain.
/// - Marks wearable / peripheral users as Incompatible for implant-only branches.
/// - Returns RiskSample only when the challenge is actually executed. [file:1]
pub fn evaluate_upgrade_request(
    player: &mut CyberneticPlayer,
    device_class: DeviceClass,
    target: &str,
    now_epoch_ns: u64,
) -> UpgradeEvaluation {
    let penalties = device_penalty_profile(device_class);

    // Hard incompatibility if the upgrade intrinsically requires implants. [file:1]
    if penalties.hard_lockout && requires_implant_only(target) {
        return UpgradeEvaluation {
            challenge_class: ChallengeClass::Incompatible,
            device_class,
            penalties,
            risk_sample: None,
            incompatibility: Some(Incompatibility {
                reason_code: 1001,
                message: "upgrade branch requires implant / cybernetic-chipset / biodegradable channel",
            }),
        };
    }

    // Consent is implicit by choosing to upgrade; here it is formalized as a hash. [file:1]
    let hc_bytes = consent_hash(&player.user_id, &player.bio_key, now_epoch_ns);
    let hc_mod = consent_hash_mod_u64(&hc_bytes);

    // Depth boost for compliance penetration: deeper integration → harder game. [file:1]
    let boosted_depth = boosted_depth_for_penetration(player.depth);
    player.depth = boosted_depth * penalties.df_multiplier;

    // Run core energy drain once, then scale risk components according to device penalties. [file:1]
    let mut sample = cybernetic_energy_drain(player, hc_mod);

    sample.sf_psych *= penalties.psych_multiplier;
    sample.ed_percent *= penalties.energy_multiplier;
    sample.risk_score = (sample.ed_percent * sample.sf_psych)
        / max(1, (hc_mod % 100_000_000) as i64) as f64;

    // Classify challenge severity for UI and governance. [file:1]
    let challenge_class = if penalties.hard_lockout {
        ChallengeClass::HardMode
    } else {
        ChallengeClass::Feasible
    };

    UpgradeEvaluation {
        challenge_class,
        device_class,
        penalties,
        risk_sample: Some(sample),
        incompatibility: None,
    }
}
