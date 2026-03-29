// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Basic VCF (Variant Call Format) parser.
//!
//! Parses standard VCF files, extracting rsid, chromosome, position, and
//! genotype from the FORMAT/SAMPLE columns.

use std::io::BufRead;

use super::parser::{ParseError, ParsedVariant};

/// Parse a VCF file, yielding one `ParsedVariant` per valid data line.
pub fn parse(reader: impl BufRead) -> impl Iterator<Item = Result<ParsedVariant, ParseError>> {
    reader.lines().filter_map(|line_result| match line_result {
        Err(e) => Some(Err(ParseError::Io(e))),
        Ok(line) => {
            let trimmed = line.trim();
            // Skip header lines
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            Some(parse_line(trimmed))
        }
    })
}

fn parse_line(line: &str) -> Result<ParsedVariant, ParseError> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < 10 {
        return Err(ParseError::InvalidLine(format!(
            "expected at least 10 tab-separated fields, got {}",
            fields.len()
        )));
    }

    let chrom = fields[0]
        .trim()
        .strip_prefix("chr")
        .unwrap_or(fields[0].trim());
    let pos_str = fields[1].trim();
    let rsid = fields[2].trim();
    let ref_allele = fields[3].trim();
    let alt_allele = fields[4].trim();
    let format = fields[8].trim();
    let sample = fields[9].trim();

    // Only process variants with an rsid
    if rsid == "." || rsid.is_empty() {
        return Err(ParseError::InvalidRsid("missing rsid".to_string()));
    }

    let position: i64 = pos_str
        .parse()
        .map_err(|_| ParseError::InvalidPosition(pos_str.to_string()))?;

    // Normalize chromosome
    let chromosome = chrom.to_string();

    // Parse genotype from FORMAT/SAMPLE columns
    let genotype = extract_genotype(format, sample, ref_allele, alt_allele)?;

    Ok(ParsedVariant {
        rsid: rsid.to_string(),
        chromosome,
        position,
        genotype,
    })
}

/// Extract genotype from VCF FORMAT and SAMPLE fields.
///
/// FORMAT is something like "GT:GQ:DP" and SAMPLE is "0/1:99:30".
/// We extract the GT (genotype) field and convert numeric indices to alleles.
fn extract_genotype(
    format: &str,
    sample: &str,
    ref_allele: &str,
    alt_allele: &str,
) -> Result<String, ParseError> {
    let format_fields: Vec<&str> = format.split(':').collect();
    let sample_fields: Vec<&str> = sample.split(':').collect();

    let gt_index = format_fields
        .iter()
        .position(|&f| f == "GT")
        .ok_or_else(|| ParseError::InvalidLine("no GT field in FORMAT".to_string()))?;

    let gt = sample_fields.get(gt_index).ok_or_else(|| {
        ParseError::InvalidLine("SAMPLE has fewer fields than FORMAT".to_string())
    })?;

    // GT is like "0/1", "1/1", "0|1", etc.
    let separator = if gt.contains('/') { '/' } else { '|' };
    let allele_indices: Vec<&str> = gt.split(separator).collect();

    let alleles: Vec<&str> = std::iter::once(ref_allele)
        .chain(alt_allele.split(','))
        .collect();

    let mut genotype = String::new();
    for idx_str in &allele_indices {
        if *idx_str == "." {
            genotype.push('-');
            continue;
        }
        let idx: usize = idx_str
            .parse()
            .map_err(|_| ParseError::InvalidGenotype(gt.to_string()))?;
        if idx >= alleles.len() {
            return Err(ParseError::InvalidGenotype(format!(
                "allele index {idx} out of range"
            )));
        }
        genotype.push_str(alleles[idx]);
    }

    // For single-character alleles, this produces e.g. "AG".
    // For multi-character (indels), just concatenate.
    Ok(genotype)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parse_basic_vcf() {
        let input = "##fileformat=VCFv4.1\n\
                     #CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tSAMPLE\n\
                     1\t82154\trs4477212\tA\tG\t.\t.\t.\tGT\t0/0\n\
                     1\t752566\trs3094315\tA\tG\t.\t.\t.\tGT\t0/1\n\
                     2\t100000\trs9999999\tC\tT\t.\t.\t.\tGT\t1/1\n";

        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();

        assert_eq!(results.len(), 3);

        let v0 = results[0].as_ref().unwrap();
        assert_eq!(v0.rsid, "rs4477212");
        assert_eq!(v0.chromosome, "1");
        assert_eq!(v0.genotype, "AA"); // 0/0 = ref/ref

        let v1 = results[1].as_ref().unwrap();
        assert_eq!(v1.genotype, "AG"); // 0/1 = ref/alt

        let v2 = results[2].as_ref().unwrap();
        assert_eq!(v2.genotype, "TT"); // 1/1 = alt/alt
    }

    #[test]
    fn skip_missing_rsid() {
        let input = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tSAMPLE\n\
                     1\t82154\t.\tA\tG\t.\t.\t.\tGT\t0/1\n";

        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn handle_chr_prefix() {
        let input = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tSAMPLE\n\
                     chr1\t82154\trs4477212\tA\tG\t.\t.\t.\tGT\t0/1\n";

        let reader = Cursor::new(input);
        let results: Vec<_> = parse(reader).collect();
        let v = results[0].as_ref().unwrap();
        assert_eq!(v.chromosome, "1");
    }

    #[test]
    fn extract_genotype_with_format() {
        let gt = extract_genotype("GT:GQ:DP", "0/1:99:30", "A", "G").unwrap();
        assert_eq!(gt, "AG");
    }
}
