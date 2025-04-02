#![allow(dead_code)]

use flate2::read::GzDecoder;
use jiff::Timestamp;
use md5::{Context, Digest};
use noodles::{bed, fasta, gff, gtf};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{RefDataset, ValidationError, data::DownloadStatus};

#[derive(Debug)]
pub(crate) enum UnvalidatedFile {
    Fasta { uri: String, local_path: PathBuf },
    Genbank { uri: String, local_path: PathBuf },
    Gfa { uri: String, local_path: PathBuf },
    Gff { uri: String, local_path: PathBuf },
    Gtf { uri: String, local_path: PathBuf },
    Bed { uri: String, local_path: PathBuf },
}

impl UnvalidatedFile {
    pub(crate) fn url(&self) -> &str {
        match self {
            UnvalidatedFile::Fasta { uri, .. }
            | UnvalidatedFile::Genbank { uri, .. }
            | UnvalidatedFile::Gfa { uri, .. }
            | UnvalidatedFile::Gff { uri, .. }
            | UnvalidatedFile::Gtf { uri, .. }
            | UnvalidatedFile::Bed { uri, .. } => uri,
        }
    }
    pub(crate) fn uri(&self) -> &str {
        self.url()
    }

    pub fn set_path(&mut self, path: PathBuf) {
        match self {
            UnvalidatedFile::Fasta { local_path, .. }
            | UnvalidatedFile::Genbank { local_path, .. }
            | UnvalidatedFile::Gfa { local_path, .. }
            | UnvalidatedFile::Gff { local_path, .. }
            | UnvalidatedFile::Gtf { local_path, .. }
            | UnvalidatedFile::Bed { local_path, .. } => *local_path = path,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
pub struct ValidatedFile {
    pub uri: String,
    pub validated: bool,
    pub hash: Option<String>,
    pub last_validated: Option<Timestamp>,
}

impl Display for ValidatedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ValidatedFile {{ uri: {}, validated: {}, hash: {}, last_validated: {} }}",
            self.uri,
            self.validated,
            self.hash.as_deref().unwrap_or("None"),
            self.last_validated
                .as_ref()
                .map_or_else(|| "None".to_string(), std::string::ToString::to_string)
        )
    }
}

impl ValidatedFile {
    pub fn checksum(&self, new_hash: Option<&str>) -> bool {
        // TODO: Must make it return false if the old_hash is None or if the old file path doesn't exist
        todo!()
    }
}

impl UnvalidatedFile {
    pub fn try_validate(&self) -> Result<ValidatedFile, ValidationError> {
        let (uri, local_path) = match self {
            UnvalidatedFile::Fasta { uri, local_path } => {
                try_parse_fasta(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Genbank { uri, local_path } => {
                try_parse_genbank(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Gfa { uri, local_path } => {
                try_parse_gfa(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Gff { uri, local_path } => {
                try_parse_gff(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Gtf { uri, local_path } => {
                try_parse_gtf(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Bed { uri, local_path } => {
                try_parse_bed(local_path)?;
                (uri, local_path)
            }
        };
        let hash = hash_valid_download(local_path).expect("");
        let timestamp = Timestamp::now();
        let validated = ValidatedFile {
            uri: uri.clone(),
            validated: true,
            hash: Some(hash),
            last_validated: Some(timestamp),
        };

        Ok(validated)
    }
}

pub fn hash_valid_download(download: impl AsRef<Path>) -> Result<String, ValidationError> {
    let Ok(file) = File::open(download.as_ref()) else {
        return Err(ValidationError::InaccessibleFile(
            "Unable to access downloaded file, indicating that file permissions may have changed."
                .to_string(),
        ));
    };
    let mut reader = BufReader::new(file);
    let mut context = Context::new();

    let mut buffer = [0u8; 64 * 1024]; // 64 KB buffer size, adjust as needed

    loop {
        let Ok(bytes_read) = reader.read(&mut buffer) else {
            return Err(ValidationError::InaccessibleFile(
                "Unable to access downloaded file, indicating that file permissions may have changed."
                    .to_string(),
            ));
        };
        if bytes_read == 0 {
            break; // EOF reached
        }
        context.consume(&buffer[..bytes_read]);
    }

    let computed: Digest = context.compute();
    let computed_hex = format!("{computed:x}");

    Ok(computed_hex)
}

#[allow(clippy::similar_names)]
pub fn validate_files(dataset: &RefDataset) -> Result<(), ValidationError> {
    #[inline]
    fn fasta_callback(dataset_fasta: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_fasta {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_fasta(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn genbank_callback(dataset_genbank: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_genbank {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => {
                    try_parse_genbank(&validated_file.uri)
                }
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn gfa_callback(dataset_gfa: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_gfa {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_gfa(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn gff_callback(dataset_gff: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_gff {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_gff(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn gtf_callback(dataset_gtf: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_gtf {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_gtf(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn bed_callback(dataset_bed: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_bed {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_bed(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    let validation_callbacks = vec![
        fasta_callback(dataset.fasta.as_ref()),
        genbank_callback(dataset.genbank.as_ref()),
        gfa_callback(dataset.gfa.as_ref()),
        gff_callback(dataset.gff.as_ref()),
        gtf_callback(dataset.gtf.as_ref()),
        bed_callback(dataset.bed.as_ref()),
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
