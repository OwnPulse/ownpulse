// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Genetic interpretation logic.
//!
//! Compares user genotypes against annotation data to compute risk levels
//! and generate personalized summaries.

use crate::models::genetics::{Interpretation, InterpretationRow};

/// Disclaimer text included with all interpretation responses.
pub const DISCLAIMER: &str = "This information is for educational purposes only and should \
    not be used for medical decisions. Consult a healthcare provider or genetic counselor \
    for clinical interpretation.";

/// Compute the risk level for a given genotype and annotation.
///
/// For pharmacogenomics variants, returns metabolizer status.
/// For other categories, returns "high", "moderate", or "low".
pub fn compute_risk_level(
    genotype: Option<&str>,
    risk_allele: Option<&str>,
    normal_allele: Option<&str>,
    category: &str,
) -> String {
    let genotype = match genotype {
        Some(g) if !g.is_empty() => g,
        _ => return "unknown".to_string(),
    };

    let risk = match risk_allele {
        Some(r) if !r.is_empty() => r,
        _ => return "unknown".to_string(),
    };

    if category == "pharmacogenomics" {
        return compute_pharma_risk(genotype, risk, normal_allele);
    }

    // Count how many alleles match the risk allele
    let genotype_chars: Vec<char> = genotype.chars().collect();
    let risk_chars: Vec<char> = risk.chars().collect();

    if risk_chars.len() == 1 {
        let risk_char = risk_chars[0];
        let risk_count = genotype_chars.iter().filter(|&&c| c == risk_char).count();

        match risk_count {
            2 => "high".to_string(),
            1 => "moderate".to_string(),
            0 => "low".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        // Multi-character risk allele (e.g., "CT") — compare directly
        if genotype == risk {
            "high".to_string()
        } else if let Some(normal) = normal_allele {
            if genotype == normal {
                "low".to_string()
            } else {
                "moderate".to_string()
            }
        } else {
            "moderate".to_string()
        }
    }
}

fn compute_pharma_risk(genotype: &str, risk_allele: &str, normal_allele: Option<&str>) -> String {
    let risk_chars: Vec<char> = risk_allele.chars().collect();

    if risk_chars.len() == 1 {
        let risk_char = risk_chars[0];
        let genotype_chars: Vec<char> = genotype.chars().collect();
        let risk_count = genotype_chars.iter().filter(|&&c| c == risk_char).count();

        match risk_count {
            2 => "poor_metabolizer".to_string(),
            1 => "intermediate".to_string(),
            0 => {
                // Check if this is a rapid metabolizer variant
                if let Some(normal) = normal_allele {
                    if genotype != normal {
                        "rapid".to_string()
                    } else {
                        "normal".to_string()
                    }
                } else {
                    "normal".to_string()
                }
            }
            _ => "unknown".to_string(),
        }
    } else if genotype == risk_allele {
        "poor_metabolizer".to_string()
    } else if normal_allele.is_some_and(|n| genotype == n) {
        "normal".to_string()
    } else {
        "intermediate".to_string()
    }
}

/// Generate a personalized summary sentence based on genotype and risk level.
pub fn personalized_summary(row: &InterpretationRow, risk_level: &str) -> String {
    let genotype = row.user_genotype.as_deref().unwrap_or("unknown");

    let gene_part = match &row.gene {
        Some(g) => format!(" in the {g} gene"),
        None => String::new(),
    };

    match risk_level {
        "high" | "poor_metabolizer" => {
            format!(
                "Your genotype is {genotype}{gene_part}. You carry two copies of the variant. {}",
                row.summary
            )
        }
        "moderate" | "intermediate" => {
            format!(
                "Your genotype is {genotype}{gene_part}. You carry one copy of the variant. {}",
                row.summary
            )
        }
        "low" | "normal" => {
            format!(
                "Your genotype is {genotype}{gene_part}. You do not carry the risk variant. \
                 This is the typical result."
            )
        }
        "rapid" => {
            format!(
                "Your genotype is {genotype}{gene_part}. You may be a rapid metabolizer. {}",
                row.summary
            )
        }
        _ => {
            format!("Your genotype is {genotype}{gene_part}. {}", row.summary)
        }
    }
}

/// Convert raw interpretation rows into fully interpreted results.
pub fn interpret(rows: Vec<InterpretationRow>) -> Vec<Interpretation> {
    rows.into_iter()
        .map(|row| {
            let risk_level = compute_risk_level(
                row.user_genotype.as_deref(),
                row.risk_allele.as_deref(),
                row.normal_allele.as_deref(),
                &row.category,
            );
            let summary = personalized_summary(&row, &risk_level);

            Interpretation {
                rsid: row.rsid,
                gene: row.gene,
                chromosome: row.chromosome,
                position: row.position,
                user_genotype: row.user_genotype,
                category: row.category,
                title: row.title,
                summary,
                risk_level,
                significance: row.significance,
                evidence_level: row.evidence_level,
                source: row.source,
                source_id: row.source_id,
                population_frequency: row.population_frequency,
                details: row.details,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homozygous_risk_is_high() {
        assert_eq!(
            compute_risk_level(Some("TT"), Some("T"), Some("CC"), "health_risk"),
            "high"
        );
    }

    #[test]
    fn heterozygous_is_moderate() {
        assert_eq!(
            compute_risk_level(Some("CT"), Some("T"), Some("CC"), "health_risk"),
            "moderate"
        );
    }

    #[test]
    fn homozygous_normal_is_low() {
        assert_eq!(
            compute_risk_level(Some("CC"), Some("T"), Some("CC"), "health_risk"),
            "low"
        );
    }

    #[test]
    fn pharma_homozygous_risk_is_poor_metabolizer() {
        assert_eq!(
            compute_risk_level(Some("AA"), Some("A"), Some("GG"), "pharmacogenomics"),
            "poor_metabolizer"
        );
    }

    #[test]
    fn pharma_heterozygous_is_intermediate() {
        assert_eq!(
            compute_risk_level(Some("AG"), Some("A"), Some("GG"), "pharmacogenomics"),
            "intermediate"
        );
    }

    #[test]
    fn pharma_normal_genotype() {
        assert_eq!(
            compute_risk_level(Some("GG"), Some("A"), Some("GG"), "pharmacogenomics"),
            "normal"
        );
    }

    #[test]
    fn missing_genotype_is_unknown() {
        assert_eq!(
            compute_risk_level(None, Some("T"), Some("CC"), "health_risk"),
            "unknown"
        );
    }

    #[test]
    fn missing_risk_allele_is_unknown() {
        assert_eq!(
            compute_risk_level(Some("CT"), None, Some("CC"), "health_risk"),
            "unknown"
        );
    }

    #[test]
    fn personalized_summary_high_risk() {
        let row = InterpretationRow {
            rsid: "rs1801133".to_string(),
            gene: Some("MTHFR".to_string()),
            chromosome: Some("1".to_string()),
            position: Some(11856378),
            user_genotype: Some("TT".to_string()),
            category: "health_risk".to_string(),
            title: "MTHFR C677T".to_string(),
            summary: "Associated with reduced folate metabolism.".to_string(),
            risk_allele: Some("T".to_string()),
            normal_allele: Some("CC".to_string()),
            significance: "risk_factor".to_string(),
            evidence_level: "strong".to_string(),
            source: "clinvar".to_string(),
            source_id: Some("3520".to_string()),
            population_frequency: Some(0.34),
            details: serde_json::json!({}),
        };

        let summary = personalized_summary(&row, "high");
        assert!(summary.contains("TT"));
        assert!(summary.contains("MTHFR"));
        assert!(summary.contains("two copies"));
    }

    #[test]
    fn personalized_summary_low_risk() {
        let row = InterpretationRow {
            rsid: "rs1801133".to_string(),
            gene: Some("MTHFR".to_string()),
            chromosome: Some("1".to_string()),
            position: Some(11856378),
            user_genotype: Some("CC".to_string()),
            category: "health_risk".to_string(),
            title: "MTHFR C677T".to_string(),
            summary: "Associated with reduced folate metabolism.".to_string(),
            risk_allele: Some("T".to_string()),
            normal_allele: Some("CC".to_string()),
            significance: "risk_factor".to_string(),
            evidence_level: "strong".to_string(),
            source: "clinvar".to_string(),
            source_id: Some("3520".to_string()),
            population_frequency: Some(0.34),
            details: serde_json::json!({}),
        };

        let summary = personalized_summary(&row, "low");
        assert!(summary.contains("CC"));
        assert!(summary.contains("do not carry"));
    }
}
