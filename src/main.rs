use criterion::{Criterion, Throughput};
use image::imageops::FilterType;
use maroontree::{
    Av2Encoder, BitDepth, Cicp, Orientation, PlanarImage, Speed, TxPart, av2_map_quality,
};
use std::fs;
use std::hint::black_box;
use std::io::Write;
use std::time::Instant;
use tealdust::Data;

fn bench_tealdust(
    c: &mut Criterion,
    obu_data: &[u8],
    width: u32,
    height: u32,
    threads: bool,
    bp: &str,
) {
    let mut settings = tealdust::Settings::default();
    settings.n_threads = if threads { 12 } else { 1 };
    settings.run_decode = true;

    let mut group = c.benchmark_group(format!(
        "in-house av2 decode {}",
        if threads {
            "multithreaded"
        } else {
            "singlethreaded"
        }
    ));
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    group.throughput(Throughput::Bytes((width as u64) * (height as u64)));

    group.bench_function(format!("bp {bp} bit-depth, {:.2}MP", megapixels), |b| {
        let mut decoder = tealdust::Decoder::open(&settings).unwrap();
        b.iter(|| {
            let _ = decoder.send_data(Some(Data::wrap(black_box(obu_data.to_vec()))));
            loop {
                match decoder.get_picture() {
                    Ok(pic) => break black_box(pic),
                    Err(e) if matches!(e, tealdust::TealdustError::Again) => {
                        decoder.send_data(None).unwrap();
                    }
                    Err(e) => panic!("tealdust error: {}", e),
                }
            }
        })
    });
    group.finish();
}

fn bench_dav2d(
    c: &mut Criterion,
    obu_data: &[u8],
    width: u32,
    height: u32,
    threads: bool,
    bp: &str,
) {
    let mut dav2d_settings = dav2d::Settings::new();
    dav2d_settings.set_n_threads(if threads { 12 } else { 1 });

    let mut group = c.benchmark_group(format!(
        "dav2d decode {}",
        if threads {
            "multithreaded"
        } else {
            "singlethreaded"
        }
    ));
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    group.throughput(Throughput::Bytes((width as u64) * (height as u64)));
    group.bench_function(format!("bp {bp} bit-depth, {:.2}MP", megapixels), |b| {
        let mut decoder = dav2d::Decoder::with_settings(&dav2d_settings).unwrap();
        b.iter(|| {
            decoder
                .send_data(black_box(obu_data.to_vec()), None, None, None)
                .unwrap();
            loop {
                match decoder.get_picture() {
                    Ok(pic) => break black_box(pic),
                    Err(e) if matches!(e, dav2d::Error::Again) => {
                        decoder.send_pending_data().unwrap();
                    }
                    Err(e) => panic!("dav2d error: {}", e),
                }
            }
        })
    });
    group.finish();
}

fn main() {
    // {
    //     let data_vec = fs::read("ob.avif").unwrap();
    //     let mut decoder = tealdust::AvifDecoder::new(&data_vec).unwrap();
    //     let image_info = decoder.image_info().unwrap();
    //     let instant = Instant::now();
    //     let image = decoder.decode().unwrap();
    // }

    let data_vec = fs::read("out10_avif.avif").unwrap();
    for _ in 0..40 {
        let mut decoder = tealdust::AvifDecoder::new(&data_vec).unwrap();
        let image_info = decoder.image_info().unwrap();
        let instant = Instant::now();
        let image = decoder.decode().unwrap();
    }
    let mut decoder = tealdust::AvifDecoder::new(&data_vec).unwrap();
    let image_info = decoder.image_info().unwrap();
    let instant = Instant::now();
    let image = decoder.decode().unwrap();
    //
    let img = image::open("./manhattan.png")
        .unwrap()
        .resize_exact(1977, 1277, FilterType::Nearest)
        .to_rgb8();
    let pimg = PlanarImage::from_interleaved_rgb(
        img.width() as usize,
        img.height() as usize,
        BitDepth::Eight,
        &img,
    )
    .unwrap();

    let av2_encoder = Av2Encoder::with_bit_depth(av2_map_quality(70), 8)
        .with_tiles(8, 8)
        .with_txpart(TxPart::ThreeWay)
        .with_rdoq_lambda(0.09)
        .with_speed(Speed::Fast)
        .with_threads(1)
        .with_cfl(true);
    let encoded = av2_encoder
        .encode_image_420(black_box(&pimg), &Cicp::srgb_ycbcr())
        .unwrap();

    let pimg10 = PlanarImage::from_interleaved_rgb(
        img.width() as usize,
        img.height() as usize,
        BitDepth::Ten,
        &img.iter().map(|&x| (x as u16) << 2).collect::<Vec<u16>>(),
    )
    .unwrap();

    let av2_encoder10 = Av2Encoder::with_bit_depth(av2_map_quality(70), 10)
        .with_tiles(8, 8)
        .with_txpart(TxPart::ThreeWay)
        .with_rdoq_lambda(0.09)
        .with_speed(Speed::Fast)
        .with_threads(1)
        .with_cfl(true);
    let encoded10 = av2_encoder10
        .encode_image_420(black_box(&pimg10), &Cicp::srgb_ycbcr())
        .unwrap();

    let obu_data = encoded.view().to_vec();
    let obu10_data = encoded10.view().to_vec();

    let mut criterion = Criterion::default().measurement_time(std::time::Duration::from_secs(10));

    bench_tealdust(
        &mut criterion,
        &obu_data,
        image.width,
        image.height,
        false,
        "8",
    );
    bench_dav2d(
        &mut criterion,
        &obu_data,
        image.width,
        image.height,
        false,
        "8",
    );
    bench_tealdust(
        &mut criterion,
        &obu_data,
        image.width,
        image.height,
        true,
        "8",
    );
    bench_dav2d(
        &mut criterion,
        &obu_data,
        image.width,
        image.height,
        true,
        "8",
    );

    bench_tealdust(
        &mut criterion,
        &obu10_data,
        image.width,
        image.height,
        false,
        "10",
    );
    bench_dav2d(
        &mut criterion,
        &obu10_data,
        image.width,
        image.height,
        false,
        "10",
    );
    bench_tealdust(
        &mut criterion,
        &obu10_data,
        image.width,
        image.height,
        true,
        "10",
    );
    bench_dav2d(
        &mut criterion,
        &obu10_data,
        image.width,
        image.height,
        true,
        "10",
    );

    criterion.final_summary();
}
