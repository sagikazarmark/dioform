#![allow(dead_code)]

use dioform::{Form, FormHandle};

#[derive(Clone, Debug, Form)]
struct SurveyForm {
    sections: Vec<Section>,
}

#[derive(Clone, Debug, Form)]
struct Section {
    questions: Vec<Question>,
}

#[derive(Clone, Debug, Form)]
struct Question {
    label: String,
}

fn main() {
    let form = FormHandle::new(SurveyForm {
        sections: vec![Section {
            questions: vec![Question {
                label: "Name".to_owned(),
            }],
        }],
    });
    let sections = form.collection(SurveyForm::fields().sections());
    let section = sections.items()[0].clone();

    section.collection(Section::fields().questions());
}
