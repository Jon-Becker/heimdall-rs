use criterion::{black_box, criterion_group, criterion_main, Criterion};
use heimdall_decompiler::ir::{
    passes, parser::Parser, tokenizer::Tokenizer, SolidityEmitter,
};
use heimdall_vm::{
    core::{
        opcodes::{ADD, MUL, PUSH1, SUB},
        vm::{Instruction, State},
    },
    ext::exec::VMTrace,
};

fn create_large_trace(size: usize) -> VMTrace {
    let mut operations = Vec::with_capacity(size);
    
    for i in 0..size {
        let instr = Instruction {
            instruction: i as u128,
            opcode: match i % 4 {
                0 => PUSH1,
                1 => ADD,
                2 => MUL,
                _ => SUB,
            },
            inputs: vec![],
            outputs: vec![alloy::primitives::U256::from(i)],
            input_operations: vec![],
            output_operations: vec![],
        };
        
        operations.push(State {
            last_instruction: instr,
            gas_used: 0,
            gas_remaining: 0,
            stack: Default::default(),
            memory: Default::default(),
            storage: Default::default(),
            events: vec![],
        });
    }
    
    VMTrace {
        instruction: 0,
        gas_used: 0,
        operations,
        children: vec![],
    }
}

fn bench_tokenizer(c: &mut Criterion) {
    let trace = create_large_trace(1000);
    
    c.bench_function("tokenizer_1000_ops", |b| {
        b.iter(|| {
            Tokenizer::tokenize(black_box(&trace))
        });
    });
}

fn bench_parser(c: &mut Criterion) {
    let trace = create_large_trace(1000);
    let tokens = Tokenizer::tokenize(&trace).unwrap();
    
    c.bench_function("parser_1000_ops", |b| {
        b.iter(|| {
            Parser::parse(black_box(tokens.clone()))
        });
    });
}

fn bench_optimization_passes(c: &mut Criterion) {
    let trace = create_large_trace(100);
    let tokens = Tokenizer::tokenize(&trace).unwrap();
    let ir = Parser::parse(tokens).unwrap();
    
    c.bench_function("optimization_passes_100_ops", |b| {
        b.iter(|| {
            passes::run_all_passes(black_box(ir.clone()))
        });
    });
}

fn bench_emitter(c: &mut Criterion) {
    let trace = create_large_trace(100);
    let tokens = Tokenizer::tokenize(&trace).unwrap();
    let ir = Parser::parse(tokens).unwrap();
    let optimized = passes::run_all_passes(ir).unwrap();
    let emitter = SolidityEmitter::new();
    
    c.bench_function("emitter_100_ops", |b| {
        b.iter(|| {
            emitter.emit(black_box(&optimized))
        });
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let trace = create_large_trace(100);
    
    c.bench_function("full_pipeline_100_ops", |b| {
        b.iter(|| {
            let tokens = Tokenizer::tokenize(black_box(&trace)).unwrap();
            let ir = Parser::parse(tokens).unwrap();
            let optimized = passes::run_all_passes(ir).unwrap();
            let emitter = SolidityEmitter::new();
            emitter.emit(&optimized)
        });
    });
}

criterion_group!(
    benches,
    bench_tokenizer,
    bench_parser,
    bench_optimization_passes,
    bench_emitter,
    bench_full_pipeline
);
criterion_main!(benches);