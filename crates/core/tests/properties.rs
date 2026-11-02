//! Property tests: random edit sequences must keep every invariant, and undo must be exact.

use proptest::prelude::*;
use tellshot_core::{
    Arrange, BaseImage, CoalesceKey, Document, Edit, History, ObjectId, Point, Rect, Shape, palette,
};

fn coord() -> impl Strategy<Value = f32> {
    -50.0_f32..450.0
}

fn point() -> impl Strategy<Value = Point> {
    (coord(), coord()).prop_map(|(x, y)| Point::new(x, y))
}

fn rect() -> impl Strategy<Value = Rect> {
    (coord(), coord(), 0.0_f32..300.0, 0.0_f32..300.0)
        .prop_map(|(x, y, w, h)| Rect::new(x, y, w, h))
}

fn shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        (point(), point()).prop_map(|(from, to)| Shape::Arrow { from, to }),
        (rect(), 0.0_f32..20.0)
            .prop_map(|(rect, corner_radius)| Shape::Rectangle { rect, corner_radius }),
        rect().prop_map(|rect| Shape::Ellipse { rect }),
        rect().prop_map(|rect| Shape::Redact { rect }),
        (rect(), 1.0_f32..40.0).prop_map(|(rect, radius)| Shape::Blur { rect, radius }),
        (point(), 4.0_f32..40.0).prop_map(|(center, radius)| Shape::Step { center, radius }),
        ("[a-zA-Z ]{0,20}", 8.0_f32..72.0).prop_map(|(text, font_size)| Shape::Text {
            origin: Point::new(5.0, 5.0),
            text,
            font_size
        }),
        prop::collection::vec(point(), 0..20).prop_map(|points| Shape::Freehand { points }),
    ]
}

/// Edits reference ids 1..=12, so many target real objects and some target missing ones.
fn edit() -> impl Strategy<Value = Edit> {
    let id = (1_u64..=12).prop_map(ObjectId);
    prop_oneof![
        3 => shape().prop_map(|shape| Edit::Add { shape, style: None }),
        1 => id.clone().prop_map(|id| Edit::Delete { id }),
        2 => (id.clone(), -20.0_f32..20.0, -20.0_f32..20.0).prop_map(|(id, dx, dy)| Edit::Translate { id, dx, dy }),
        1 => (id.clone(), shape()).prop_map(|(id, shape)| Edit::SetShape { id, shape }),
        1 => id.clone().prop_map(|id| Edit::SetStyle {
            id,
            style: tellshot_core::Style { stroke: palette::BLUE, fill: None, stroke_width: 3.0 },
        }),
        1 => (id, prop_oneof![Just(Arrange::Front), Just(Arrange::Back), Just(Arrange::Forward), Just(Arrange::Backward)])
            .prop_map(|(id, to)| Edit::Arrange { id, to }),
        1 => prop::option::of(rect()).prop_map(|rect| Edit::Crop { rect }),
    ]
}

fn content(doc: &Document) -> (Vec<tellshot_core::Object>, Option<Rect>) {
    (doc.objects().to_vec(), doc.crop())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn undo_all_restores_and_redo_all_replays(
        edits in prop::collection::vec((edit(), prop::option::of(0_u64..3)), 0..60)
    ) {
        let mut doc = Document::new(BaseImage { width: 400, height: 300 });
        let mut history = History::new();
        let initial = content(&doc);
        let mut applied = 0_usize;
        for (edit, key) in edits {
            if let Ok(Some(_)) = history.apply(&mut doc, edit, key.map(CoalesceKey)) {
                applied += 1;
            }
            prop_assert_eq!(doc.validate(), Ok(()));
        }
        let finished = content(&doc);

        let mut undone = 0_usize;
        while history.undo(&mut doc).expect("undo never mismatches").is_some() {
            undone += 1;
            prop_assert_eq!(doc.validate(), Ok(()));
        }
        prop_assert!(undone <= applied, "coalescing can only reduce undo steps");
        prop_assert_eq!(content(&doc), initial);

        while history.redo(&mut doc).expect("redo never mismatches").is_some() {}
        prop_assert_eq!(content(&doc), finished);
    }

    #[test]
    fn documents_survive_json_round_trips(edits in prop::collection::vec(edit(), 0..30)) {
        let mut doc = Document::new(BaseImage { width: 400, height: 300 });
        let mut history = History::new();
        for edit in edits {
            let _ignored = history.apply(&mut doc, edit, None);
        }
        let json = serde_json::to_string(&doc).expect("serialise");
        let back: Document = serde_json::from_str(&json).expect("deserialise");
        prop_assert_eq!(back.validate(), Ok(()));
        prop_assert_eq!(back, doc);
    }

    #[test]
    fn arbitrary_json_never_panics(input in ".{0,400}") {
        // Agents and files send arbitrary bytes; parsing must fail cleanly, never panic.
        if let Ok(doc) = serde_json::from_str::<Document>(&input) {
            let _checked = doc.validate();
        }
        let _parsed: Result<Edit, _> = serde_json::from_str(&input);
    }
}
