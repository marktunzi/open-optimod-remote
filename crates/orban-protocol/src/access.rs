//! What a PC Remote session may change at its access level. The 5700i
//! levels are those of `5700iPC.exe` 3.0.1.20: its menu update handlers
//! compare the login's level with these bounds (Recall 0x4382e0, Save/Save
//! As/Delete 0x437ef0/0x437a00/0x438af0, Setup 0x438cb0, Automation
//! 0x43abd0, Security 0x43aca0, Backup/Restore 0x43ad70/0x43add0); the level
//! names are those of its Security dialog. Other models keep full access at
//! level 0 and read-only access otherwise until their clients are compared.
use crate::adapter::DeviceModel;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Access {
    pub level: u8,
    /// Recall a preset.
    pub recall: bool,
    /// Change processing (Modify).
    pub modify: bool,
    /// Save, Save As, Rename and Delete user presets.
    pub presets: bool,
    /// Change System Setup.
    pub setup: bool,
    pub automation: bool,
    pub security: bool,
    /// Back up and restore (the client's Memory).
    pub memory: bool,
}

pub fn access(model: DeviceModel, level: u8) -> Access {
    if model == DeviceModel::Optimod5700i {
        // 0 All screens, 1 … except Security, 2 … except Modify and Security,
        // 3 Presets, Modify, Save, Memory and Automation, 4 Presets and
        // Automation, 5 Presets. Level 3's "Memory" is not Backup/Restore:
        // those menus need level 1 or lower.
        return Access {
            level,
            recall: level <= 5,
            modify: matches!(level, 0 | 1 | 3),
            presets: matches!(level, 0 | 1 | 3),
            setup: level <= 2,
            automation: level <= 4,
            security: level == 0,
            memory: level <= 1,
        };
    }
    let full = level == 0;
    Access {
        level,
        recall: full,
        modify: full,
        presets: full,
        setup: full,
        automation: full,
        security: full,
        memory: full,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_5700i_client() {
        let a = |level| access(DeviceModel::Optimod5700i, level);
        assert_eq!(
            a(0),
            Access {
                level: 0,
                recall: true,
                modify: true,
                presets: true,
                setup: true,
                automation: true,
                security: true,
                memory: true
            }
        );
        assert!(a(1).memory && !a(1).security);
        assert!(a(2).setup && !a(2).modify && !a(2).presets);
        assert!(a(3).modify && a(3).presets && !a(3).setup && !a(3).memory && a(3).automation);
        assert!(a(4).recall && a(4).automation && !a(4).modify);
        assert!(a(5).recall && !a(5).automation && !a(5).setup);
        assert!(!access(DeviceModel::Optimod8700Hd, 1).recall);
    }
}
