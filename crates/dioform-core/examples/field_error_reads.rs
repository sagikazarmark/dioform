use std::{hint::black_box, time::Instant};

use dioform_core::{FieldIdentity, FormCore, ValidationTrigger};

#[derive(Clone, Debug, Eq, PartialEq)]
struct BenchmarkForm;

fn main() {
    println!("registered validators | nanoseconds per visible field read");

    for validator_count in [10, 100, 1_030] {
        measure(validator_count);
    }
}

fn measure(validator_count: usize) {
    let target = FieldIdentity::new("target");
    let mut form: FormCore<BenchmarkForm, &'static str> =
        FormCore::new_with_error_type(BenchmarkForm);

    form.register_sync_field_identity_validator_for_triggers(
        target.clone(),
        "target",
        ValidationTrigger::Manual,
        |_model, _context| vec!["invalid"],
    );

    for index in 1..validator_count {
        form.register_sync_field_identity_validator_for_triggers(
            FieldIdentity::new(format!("unrelated-{index:04}")),
            format!("unrelated-{index:04}"),
            ValidationTrigger::Manual,
            |_model, _context| Vec::new(),
        );
    }

    form.validate_all(ValidationTrigger::Manual);
    form.mark_field_identity_blurred(&target);

    for _ in 0..10_000 {
        black_box(form.visible_field_validation_errors_by_identity(&target));
    }

    let reads = 1_000_000;
    let started = Instant::now();

    for _ in 0..reads {
        black_box(form.visible_field_validation_errors_by_identity(&target));
    }

    let nanoseconds_per_read = started.elapsed().as_nanos() as f64 / reads as f64;
    println!("{validator_count:>21} | {nanoseconds_per_read:>34.1}");
}
