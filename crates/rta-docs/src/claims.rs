//! What SITL shows. A test name stays in its column.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Shown,
    NotShown,
}

#[derive(Debug, Clone, Copy)]
pub struct Claim {
    pub column: Column,
    pub item: &'static str,
}

pub const CLAIMS: &[Claim] = &[
    Claim {
        column: Column::Shown,
        item: "scripts/sitl.sh",
    },
    Claim {
        column: Column::Shown,
        item: "scripts/sitl-trips.sh",
    },
    Claim {
        column: Column::Shown,
        item: "scripts/sitl-px4.sh",
    },
    Claim {
        column: Column::NotShown,
        item: "Target timing",
    },
    Claim {
        column: Column::NotShown,
        item: "Aerodynamic truth",
    },
    Claim {
        column: Column::NotShown,
        item: "Tracker performance",
    },
];

#[cfg(test)]
mod tests {
    use super::{Claim, Column, CLAIMS};

    #[test]
    fn each_m9_script_is_shown_and_target_timing_is_not() {
        let doc = include_str!("../../../docs/sitl.md");
        let shown: Vec<&Claim> = CLAIMS
            .iter()
            .filter(|c| c.column == Column::Shown)
            .collect();
        let hidden: Vec<&Claim> = CLAIMS
            .iter()
            .filter(|c| c.column == Column::NotShown)
            .collect();
        for claim in &shown {
            assert!(doc.contains(claim.item), "{}", claim.item);
        }
        assert!(hidden.iter().any(|c| c.item == "Target timing"));
        assert!(doc.contains("Target timing"));
        assert!(doc.contains("Not shown"));
        let shown_block = doc.split("Not shown").next().unwrap();
        assert!(!shown_block.contains("Target timing"));
    }
}
