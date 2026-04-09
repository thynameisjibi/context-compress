//! Compression benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use context_compress_core::{TokenCounter, ExtractiveCompressor, AbstractiveCompressor, HybridCompressor};

fn benchmark_token_counting(c: &mut Criterion) {
    let counter = TokenCounter::default();
    let text = "This is a sample text for benchmarking token counting performance. ".repeat(100);
    
    c.bench_function("token_count_1k", |b| {
        b.iter(|| counter.count(black_box(&text)))
    });
}

fn benchmark_extractive_compression(c: &mut Criterion) {
    let compressor = ExtractiveCompressor::default();
    let text = "This is sentence one. This is sentence two. This is sentence three. ".repeat(50);
    
    c.bench_function("extractive_compress", |b| {
        b.iter(|| compressor.compress(black_box(&text)))
    });
}

fn benchmark_abstractive_compression(c: &mut Criterion) {
    let compressor = AbstractiveCompressor::default();
    let text = "This is a sample text for benchmarking abstractive compression. ".repeat(50);
    
    c.bench_function("abstractive_compress", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(compressor.compress(black_box(&text)))
        })
    });
}

fn benchmark_hybrid_compression(c: &mut Criterion) {
    let compressor = HybridCompressor::default()
        .with_abstractive(AbstractiveCompressor::default());
    let text = "This is a sample text for benchmarking hybrid compression. ".repeat(50);
    
    c.bench_function("hybrid_compress", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(compressor.compress(black_box(&text)))
        })
    });
}

fn benchmark_compression_sizes(c: &mut Criterion) {
    let compressor = ExtractiveCompressor::default();
    let mut group = c.benchmark_group("compression_by_size");
    
    for size in [10, 50, 100, 500].iter() {
        let text = "This is a sentence for testing different sizes. ".repeat(*size);
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &text,
            |b, text| {
                b.iter(|| compressor.compress(black_box(text)))
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_token_counting,
    benchmark_extractive_compression,
    benchmark_abstractive_compression,
    benchmark_hybrid_compression,
    benchmark_compression_sizes,
);
criterion_main!(benches);
