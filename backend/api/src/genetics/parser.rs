// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Unified parser interface for genetic file formats.

use std::fmt;
use std::io::BufRead;

use crate::error::ApiError;

/// Supported genetic file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneticFileFormat {
    TwentyThreeAndMe,
    AncestryDNA,
    VCF,
}

impl fmt::Display for GeneticFileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TwentyThreeAndMe => write!(f, "23andme"),
            Self::AncestryDNA => write!(f, "ancestrydna"),
            Self::VCF => write!(f, "vcf"),
        }
    }
}

impl GeneticFileFormat {
    /// Human-readable source name for the `genetic_records.source` column.
    pub fn source_name(&self) -> &'static str {
        match self {
            Self::TwentyThreeAndMe => "23andMe",
            Self::AncestryDNA => "AncestryDNA",
            Self::VCF => "VCF",
        }
    }
}

/// A single parsed genetic variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVariant {
    pub rsid: String,
    pub chromosome: String,
    pub position: i64,
    pub genotype: String,
}

/// Errors during genetic file parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid line format: {0}")]
    InvalidLine(String),
    #[error("invalid rsid: {0}")]
    InvalidRsid(String),
    #[error("invalid chromosome: {0}")]
    InvalidChromosome(String),
    #[error("invalid position: {0}")]
    InvalidPosition(String),
    #[error("invalid genotype: {0}")]
    InvalidGenotype(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Detect the file format from the first few lines of content.
pub fn detect_format(first_lines: &[String]) -> Result<GeneticFileFormat, ApiError> {
    for line in first_lines {
        let trimmed = line.trim();

        // VCF files start with ##fileformat=VCF
        if trimmed.starts_with("##fileformat=VCF") {
            return Ok(GeneticFileFormat::VCF);
        }

        // VCF header lines
        if trimmed.starts_with("##") {
            continue;
        }

        // 23andMe files have comment lines starting with #
        // and a specific header format
        if trimmed.starts_with("# rsid") || trimmed.starts_with("#rsid") {
            return Ok(GeneticFileFormat::TwentyThreeAndMe);
        }

        // 23andMe comment lines
        if trimmed.starts_with('#') {
            continue;
        }

        // AncestryDNA header line
        if trimmed.starts_with("rsid\tchromosome\tposition\tallele1\tallele2")
            || trimmed.starts_with("rsid\tchr\tpos\tallele1\tallele2")
        {
            return Ok(GeneticFileFormat::AncestryDNA);
        }

        // VCF data header
        if trimmed.starts_with("#CHROM") {
            return Ok(GeneticFileFormat::VCF);
        }

        // Try to detect from data lines — AncestryDNA has 5 tab-separated fields
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() == 5
            && fields[0].starts_with("rs")
            && fields[3].len() <= 2
            && fields[4].len() <= 2
        {
            return Ok(GeneticFileFormat::AncestryDNA);
        }

        // 23andMe data: 4 tab-separated fields
        if fields.len() == 4 && (fields[0].starts_with("rs") || fields[0].starts_with('i')) {
            return Ok(GeneticFileFormat::TwentyThreeAndMe);
        }
    }

    Err(ApiError::BadRequest(
        "unable to detect genetic file format".to_string(),
    ))
}

/// Validate that a chromosome value is acceptable.
pub fn is_valid_chromosome(chr: &str) -> bool {
    matches!(
        chr,
        "1" | "2"
            | "3"
            | "4"
            | "5"
            | "6"
            | "7"
            | "8"
            | "9"
            | "10"
            | "11"
            | "12"
            | "13"
            | "14"
            | "15"
            | "16"
            | "17"
            | "18"
            | "19"
            | "20"
            | "21"
            | "22"
            | "X"
            | "Y"
            | "MT"
    )
}

/// Validate that a genotype string contains only valid characters.
pub fn is_valid_genotype(genotype: &str) -> bool {
    !genotype.is_empty()
        && genotype.len() <= 2
        && genotype
            .chars()
            .all(|c| matches!(c, 'A' | 'C' | 'G' | 'T' | '-'))
}

/// Parse a genetic file with streaming, yielding batches of variants.
pub fn parse_stream(
    reader: impl BufRead,
    format: GeneticFileFormat,
) -> Vec<Result<ParsedVariant, ParseError>> {
    match format {
        GeneticFileFormat::TwentyThreeAndMe => super::twentythreeandme::parse(reader).collect(),
        GeneticFileFormat::AncestryDNA => super::ancestrydna::parse(reader).collect(),
        GeneticFileFormat::VCF => super::vcf::parse(reader).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_23andme_from_header() {
        let lines = vec![
            "# This data file generated by 23andMe".to_string(),
            "# rsid\tchromosome\tposition\tgenotype".to_string(),
            "rs4477212\t1\t82154\tAA".to_string(),
        ];
        assert_eq!(
            detect_format(&lines).unwrap(),
            GeneticFileFormat::TwentyThreeAndMe
        );
    }

    #[test]
    fn detect_ancestrydna_from_header() {
        let lines = vec![
            "rsid\tchromosome\tposition\tallele1\tallele2".to_string(),
            "rs4477212\t1\t82154\tA\tA".to_string(),
        ];
        assert_eq!(
            detect_format(&lines).unwrap(),
            GeneticFileFormat::AncestryDNA
        );
    }

    #[test]
    fn detect_vcf_from_header() {
        let lines = vec![
            "##fileformat=VCFv4.1".to_string(),
            "##source=23andMe".to_string(),
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tSAMPLE".to_string(),
        ];
        assert_eq!(detect_format(&lines).unwrap(), GeneticFileFormat::VCF);
    }

    #[test]
    fn detect_format_unknown_returns_error() {
        let lines = vec!["random garbage data".to_string()];
        assert!(detect_format(&lines).is_err());
    }

    #[test]
    fn valid_chromosomes() {
        for chr in &["1", "2", "10", "22", "X", "Y", "MT"] {
            assert!(is_valid_chromosome(chr), "expected {chr} to be valid");
        }
        assert!(!is_valid_chromosome("23"));
        assert!(!is_valid_chromosome("0"));
        assert!(!is_valid_chromosome(""));
    }

    #[test]
    fn valid_genotypes() {
        assert!(is_valid_genotype("AA"));
        assert!(is_valid_genotype("CT"));
        assert!(is_valid_genotype("A"));
        assert!(is_valid_genotype("-"));
        assert!(!is_valid_genotype(""));
        assert!(!is_valid_genotype("AAA"));
        assert!(!is_valid_genotype("AX"));
    }
}
