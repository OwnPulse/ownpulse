// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Parser for AncestryDNA raw data files.
//!
//! Format: tab-separated, 5 columns: rsid, chromosome, position, allele1, allele2.
//! First line is a header row.

use std::io::BufRead;

use super::parser::{ParseError, ParsedVariant, is_valid_chromosome, is_valid_genotype};

/// Parse an AncestryDNA file, yielding one `ParsedVariant` per valid data line.
pub fn parse(reader: impl BufRead) -> impl Iterator<Item = Result<ParsedVariant, ParseError>> {
    reader.lines().filter_map(|line_result| {
        match line_result {
            Err(e) => Some(Err(ParseError::Io(e))),
            Ok(line) => {
                let trimmed = line.trim();
                // Skip empty lines, comments, and header
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("rsid") {
                    return None;
                }
                Some(parse_line(trimmed))
            }
        }
    })
}

fn parse_line(line: &str) -> Result<ParsedVariant, ParseError> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != 5 {
        return Err(ParseError::InvalidLine(format!(
            "expected 5 tab-separated fields, got {}",
            fields.len()
        )));
    }

    let rsid = fields[0].trim();
    let chromosome = fields[1].trim();
    let position_str = fields[2].trim();
    let allele1 = fields[3].trim();
    let allele2 = fields[4].trim();

    if !rsid.starts_with("rs") && !rsid.starts_with('i') {
        return Err(ParseError::InvalidRsid(rsid.to_string()));
    }

    if !is_valid_chromosome(chromosome) {
        return Err(ParseError::InvalidChromosome(chromosome.to_string()));
    }

    let position: i64 = position_str
        .parse()
        .map_err(|_| ParseError::InvalidPosition(position_str.to_string()))?;

    // Combine alleles into a genotype string
    let genotype = format!("{allele1}{allele2}");
    if !is_valid_genotype(&genotype) {
        return Err(ParseError::InvalidGenotype(genotype));
    }

    Ok(ParsedVariant {
        rsid: rsid.to_string(),
        chromosome: chromosome.to_string(),
        position,
        genotype,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parse_valid_file() {
        let input = "rsid\tchromosome\tposition\tallele1\tallele2\n\
                     rs4477212\t1\t82154\tA\tA\n\
                     rs3094315\t1\t752566\tA\tG\n";

        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();

        assert_eq!(results.len(), 2);
        let v0 = results[0].as_ref().unwrap();
        assert_eq!(v0.rsid, "rs4477212");
        assert_eq!(v0.genotype, "AA");

        let v1 = results[1].as_ref().unwrap();
        assert_eq!(v1.rsid, "rs3094315");
        assert_eq!(v1.genotype, "AG");
    }

    #[test]
    fn skip_header_line() {
        let input = "rsid\tchromosome\tposition\tallele1\tallele2\n\
                     rs1234\t1\t100\tC\tT\n";

        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap().genotype, "CT");
    }

    #[test]
    fn allele_combination() {
        let input = "rsid\tchr\tpos\tallele1\tallele2\n\
                     rs5678\t2\t200\tG\tC\n";

        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().genotype, "GC");
    }

    #[test]
    fn invalid_field_count() {
        let input = "rs1234\t1\t100\tA\n";
        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();
        assert!(results[0].is_err());
    }
}
