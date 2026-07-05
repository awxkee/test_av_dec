use std::{
    any::Any,
    fs, io,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct AvifProbeRecord {
    pub path: PathBuf,
    pub status: AvifProbeStatus,
}

#[derive(Debug)]
pub enum AvifProbeStatus {
    Ok { bytes: usize, decode_time: Duration },
    ReadErr(String),
    DecoderOpenErr(String),
    ImageInfoErr(String),
    DecodeErr(String),
    Panic(String),
}

impl AvifProbeStatus {
    #[inline]
    pub fn is_ok(&self) -> bool {
        matches!(self, AvifProbeStatus::Ok { .. })
    }
}

pub fn probe_avif_folder(folder: impl AsRef<Path>) -> io::Result<Vec<AvifProbeRecord>> {
    let paths = collect_avifs(folder.as_ref())?;
    let total = paths.len();

    eprintln!("Scanning {total} AVIF files in {:?}", folder.as_ref());

    let started = Instant::now();
    let mut records = Vec::with_capacity(total);

    let mut ok = 0usize;
    let mut bad = 0usize;

    for (idx, path) in paths.into_iter().enumerate() {
        let file_no = idx + 1;
        let file_started = Instant::now();

        let status = catch_unwind(AssertUnwindSafe(|| probe_one_avif(&path)))
            .unwrap_or_else(|payload| AvifProbeStatus::Panic(panic_payload_to_string(payload)));

        if status.is_ok() {
            ok += 1;
        } else {
            bad += 1;
            eprintln!(
                "[{file_no}/{total}] BAD {:?}: {:?}",
                path.file_name().unwrap_or_default(),
                status
            );
        }

        // Progress: print every 25 files, every failure, and the last file.
        if file_no == total || bad > 0 && !status.is_ok() || file_no % 25 == 0 {
            let elapsed = started.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 {
                file_no as f64 / elapsed
            } else {
                0.0
            };

            eprintln!(
                "[{file_no}/{total}] ok={ok} bad={bad} last={:?} last_time={:?} rate={rate:.2}/s",
                path.file_name().unwrap_or_default(),
                file_started.elapsed(),
            );
        }

        records.push(AvifProbeRecord { path, status });
    }

    eprintln!(
        "Finished AVIF scan: total={total} ok={ok} bad={bad} elapsed={:?}",
        started.elapsed()
    );

    Ok(records)
}

fn probe_one_avif(path: &Path) -> AvifProbeStatus {
    let data_vec = match fs::read(path) {
        Ok(data) => data,
        Err(e) => return AvifProbeStatus::ReadErr(format!("{e:?}")),
    };

    let mut decoder = match tealdust::AvifDecoder::new(&data_vec) {
        Ok(decoder) => decoder,
        Err(e) => return AvifProbeStatus::DecoderOpenErr(format!("{e:?}")),
    };

    if let Err(e) = decoder.image_info() {
        return AvifProbeStatus::ImageInfoErr(format!("{e:?}"));
    }

    let instant = Instant::now();

    match decoder.decode() {
        Ok(_image) => AvifProbeStatus::Ok {
            bytes: data_vec.len(),
            decode_time: instant.elapsed(),
        },
        Err(e) => AvifProbeStatus::DecodeErr(format!("{e:?}")),
    }
}

fn collect_avifs(folder: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![folder.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let ty = entry.file_type()?;

            if ty.is_dir() {
                stack.push(path);
            } else if ty.is_file() && is_avif(&path) {
                out.push(path);
            }
        }
    }

    out.sort();
    Ok(out)
}

fn is_avif(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("avif"))
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
