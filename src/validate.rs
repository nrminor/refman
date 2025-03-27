#![allow(dead_code)]

use std::{fs::File, io::BufReader, path::Path};

use flate2::read::GzDecoder;
use jiff::Timestamp;
use noodles::{bed, fasta, gff, gtf};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{RefDataset, ValidationError};

pub struct ValidationSwitchboard {
    num_threads: i8,
}

impl Default for ValidationSwitchboard {
    fn default() -> Self {
        let num_threads = 4;
        Self { num_threads }
    }
}

#[derive(Debug)]
pub enum UnvalidatedFile<'a> {
    Fasta { uri: &'a str, local_path: &'a Path },
    Genbank { uri: &'a str, local_path: &'a Path },
    GFA { uri: &'a str, local_path: &'a Path },
    GFF { uri: &'a str, local_path: &'a Path },
    GTF { uri: &'a str, local_path: &'a Path },
    BED { uri: &'a str, local_path: &'a Path },
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ValidatedFile {
    pub uri: String,
    pub validated: bool,
    pub hash: Option<String>,
    pub last_validated: Option<Timestamp>,
}

impl ValidatedFile {
    pub fn try_validate(unvalidated: &UnvalidatedFile) -> Result<Self, ValidationError> {
        let (uri, local_path) = match unvalidated {
            UnvalidatedFile::Fasta { uri, local_path } => {
                try_parse_fasta(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Genbank { uri, local_path } => {
                try_parse_genbank(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::GFA { uri, local_path } => {
                try_parse_gfa(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::GFF { uri, local_path } => {
                try_parse_gff(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::GTF { uri, local_path } => {
                try_parse_gtf(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::BED { uri, local_path } => {
                try_parse_bed(local_path)?;
                (uri, local_path)
            }
        };
        let hash = hash_valid_download(local_path);
        let timestamp = Timestamp::now();
        let validated = Self {
            uri: String::from(*uri),
            validated: true,
            hash: Some(hash),
            last_validated: Some(timestamp),
        };

        Ok(validated)
    }
}

pub fn hash_valid_download(download: impl AsRef<Path>) -> String {
    todo!()
}

#[allow(clippy::similar_names)]
pub fn validate_files(dataset: &RefDataset) -> Result<(), ValidationError> {
    #[inline]
    fn fasta_callback(dataset_fasta: Option<&str>) -> Result<(), ValidationError> {
        match dataset_fasta {
            Some(file) => try_parse_fasta(file),
            None => Ok(()),
        }
    }
    #[inline]
    fn genbank_callback(dataset_genbank: Option<&str>) -> Result<(), ValidationError> {
        match dataset_genbank {
            Some(file) => try_parse_genbank(file),
            None => Ok(()),
        }
    }
    #[inline]
    fn gfa_callback(dataset_gfa: Option<&str>) -> Result<(), ValidationError> {
        match dataset_gfa {
            Some(file) => try_parse_gfa(file),
            None => Ok(()),
        }
    }
    #[inline]
    fn gff_callback(dataset_gff: Option<&str>) -> Result<(), ValidationError> {
        match dataset_gff {
            Some(file) => try_parse_gff(file),
            None => Ok(()),
        }
    }
    #[inline]
    fn gtf_callback(dataset_gtf: Option<&str>) -> Result<(), ValidationError> {
        match dataset_gtf {
            Some(file) => try_parse_gtf(file),
            None => Ok(()),
        }
    }
    #[inline]
    fn bed_callback(dataset_bed: Option<&str>) -> Result<(), ValidationError> {
        match dataset_bed {
            Some(file) => try_parse_bed(file),
            None => Ok(()),
        }
    }
    let validation_callbacks = vec![
        fasta_callback(dataset.fasta.as_deref()),
        genbank_callback(dataset.genbank.as_deref()),
        gfa_callback(dataset.gfa.as_deref()),
        gff_callback(dataset.gff.as_deref()),
        gtf_callback(dataset.gtf.as_deref()),
        bed_callback(dataset.bed.as_deref()),
    ]
    .into_par_iter()
    .filter_map(std::result::Result::err)
    .collect::<Vec<ValidationError>>();

    if !validation_callbacks.is_empty() {
        return Err(ValidationError::MultipleErrors(
            crate::MultipleValidationErrors(validation_callbacks),
        ));
    }

    Ok(())
}

fn try_parse_fasta(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    if file.as_ref().ends_with(".fasta") {
        let Ok(mut fa_reader) = File::open(file.as_ref())
            .map(BufReader::new)
            .map(fasta::Reader::new)
        else {
            return Err(ValidationError::InaccessibleFile(
                file.as_ref().to_string_lossy().into_owned(),
            ));
        };
        while let Some(record) = fa_reader.records().next() {
            match record {
                Ok(_) => continue,
                Err(msg) => return Err(ValidationError::InvalidFasta(format!("{msg}"))),
            }
        }
    } else if file.as_ref().extension().is_some_and(|ext| ext == "gz") {
        let Ok(mut fa_reader) = File::open(file.as_ref())
            .map(BufReader::new)
            .map(GzDecoder::new)
            .map(BufReader::new)
            .map(fasta::Reader::new)
        else {
            return Err(ValidationError::InaccessibleFile(
                file.as_ref().to_string_lossy().into_owned(),
            ));
        };
        while let Some(record) = fa_reader.records().next() {
            match record {
                Ok(_) => continue,
                Err(msg) => return Err(ValidationError::InvalidFasta(format!("{msg}"))),
            }
        }
    }
    Ok(())
}

fn try_parse_genbank(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(gbk_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(gb_io::reader::SeqReader::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };

    for record in gbk_reader {
        match record {
            Ok(_) => continue,
            Err(msg) => return Err(ValidationError::InvalidGenbank(format!("{msg}"))),
        }
    }

    Ok(())
}

fn try_parse_gfa(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(_) = gfa::parser::GFAParserBuilder::all()
        .pedantic_errors()
        .segments(false)
        .build_bstr_id::<()>()
        .parse_file(&file)
    else {
        return Err(ValidationError::InvalidGFA(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };

    Ok(())
}

fn try_parse_gff(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(mut gff_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(gff::Reader::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };
    while let Some(record) = gff_reader.record_bufs().next() {
        match record {
            Ok(_) => continue,
            Err(msg) => return Err(ValidationError::InvalidGFF(format!("{msg}"))),
        }
    }
    Ok(())
}

fn try_parse_gtf(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(mut gff_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(gtf::Reader::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };
    while let Some(record) = gff_reader.record_bufs().next() {
        match record {
            Ok(_) => continue,
            Err(msg) => return Err(ValidationError::InvalidGTF(format!("{msg}"))),
        }
    }
    Ok(())
}

fn try_parse_bed(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(mut bed_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(bed::Reader::<3, _>::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };
    let mut record = bed::Record::default();
    match bed_reader.read_record(&mut record) {
        Ok(_) => Ok(()),
        Err(msg) => Err(ValidationError::InvalidBED(format!("{msg}"))),
    }
}
