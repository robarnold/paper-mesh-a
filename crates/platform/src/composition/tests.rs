use super::*;
use embedded_graphics::mock_display::MockDisplay;
fn rect(x: i32, y: i32, w: u32, h: u32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}
fn setup() -> (Layers<8>, LayerId, LayerId) {
    let mut l = Layers::new();
    let root = l.initialize(rect(0, 0, 64, 64)).unwrap();
    let mut t = l.begin(root).unwrap();
    let owner = t.create_root().unwrap();
    t.set(
        owner,
        Properties {
            bounds: rect(0, 0, 64, 64),
            ..Properties::default()
        },
    )
    .unwrap();
    t.replace_children(root, &[owner]).unwrap();
    t.commit().unwrap();
    (l, root, owner)
}
fn button(l: &mut Layers<8>, owner: LayerId) -> LayerId {
    let mut t = l.begin(owner).unwrap();
    let b = t.create().unwrap();
    t.set(
        b,
        Properties {
            bounds: rect(4, 4, 20, 20),
            bindings: Bindings {
                tap: Some(Handler {
                    owner,
                    slot: 7,
                    generation: 0,
                }),
                pan: None,
            },
            ..Properties::default()
        },
    )
    .unwrap();
    t.replace_children(owner, &[b]).unwrap();
    t.commit().unwrap();
    b
}
fn tap(l: &mut Layers<8>, p: Point) -> Option<Delivery> {
    l.touch(TouchEvent::touch(0, p));
    l.touch(TouchEvent::release(0, p))
}
// Failed batches leave both rendering properties and revision unchanged.
#[test]
fn atomic_rollback_and_noop() {
    let (mut l, _, owner) = setup();
    let b = button(&mut l, owner);
    let r = l.revision();
    let original = l.properties(b).unwrap().clone();
    let mut t = l.begin(owner).unwrap();
    let mut changed = original.clone();
    changed.visible = false;
    t.set(b, changed).unwrap();
    assert_eq!(t.replace_children(b, &[owner]), Err(Error::Cycle));
    assert_eq!(t.commit(), Err(Error::Poisoned));
    assert_eq!(l.revision(), r);
    assert_eq!(l.properties(b).unwrap(), &original);
    assert_eq!(l.begin(owner).unwrap().commit().unwrap(), r);
}
// A scoped owner cannot modify another owner's layers, including detached content.
#[test]
fn scoped_ownership() {
    let (mut l, root, owner) = setup();
    let mut t = l.begin(root).unwrap();
    let other = t.create_root().unwrap();
    t.commit().unwrap();
    let mut t = l.begin(owner).unwrap();
    assert_eq!(t.set(other, Properties::default()), Err(Error::Ownership));
    assert_eq!(t.commit(), Err(Error::Poisoned));
}
// Detach preserves identity; destroy invalidates it even when its slot is reused.
#[test]
fn detach_destroy_and_generation() {
    let (mut l, _, owner) = setup();
    let b = button(&mut l, owner);
    let mut t = l.begin(owner).unwrap();
    t.replace_children(owner, &[]).unwrap();
    t.commit().unwrap();
    assert!(tap(&mut l, Point::new(8, 8)).is_none());
    let mut t = l.begin(owner).unwrap();
    t.replace_children(owner, &[b]).unwrap();
    t.commit().unwrap();
    assert!(tap(&mut l, Point::new(8, 8)).is_some());
    let mut t = l.begin(owner).unwrap();
    t.destroy(b).unwrap();
    let replacement = t.create().unwrap();
    assert_ne!(replacement, b);
    assert_eq!(t.set(b, Properties::default()), Err(Error::StaleHandle));
    assert_eq!(t.commit(), Err(Error::Poisoned));
}
// Committed bindings take effect before any frame submission or completion.
#[test]
fn latest_input_and_revision_tracking() {
    let (mut l, _, owner) = setup();
    let b = button(&mut l, owner);
    assert_eq!(tap(&mut l, Point::new(8, 8)).unwrap().handler.slot, 7);
    l.submitted();
    let old = l.revision();
    let mut t = l.begin(owner).unwrap();
    let mut p = t.properties(b).unwrap().clone();
    p.bounds = rect(30, 30, 20, 20);
    p.bindings.tap.as_mut().unwrap().slot = 9;
    t.set(b, p).unwrap();
    t.commit().unwrap();
    l.completed();
    assert_eq!(l.presented_revision(), old);
    assert!(l.needs_display());
    assert!(tap(&mut l, Point::new(8, 8)).is_none());
    assert_eq!(tap(&mut l, Point::new(32, 32)).unwrap().handler.slot, 9);
    l.submitted();
    l.completed();
    assert!(!l.needs_display());
    l.invalidate();
    assert!(l.needs_display());
}
// An ancestor pan captures movement and suppresses the child's tap.
#[test]
fn pan_capture_and_cancel() {
    let (mut l, _, owner) = setup();
    let b = button(&mut l, owner);
    let mut t = l.begin(owner).unwrap();
    let mut p = t.properties(owner).unwrap().clone();
    p.bindings.pan = Some(Handler {
        owner,
        slot: 2,
        generation: 0,
    });
    t.set(owner, p).unwrap();
    t.commit().unwrap();
    assert_eq!(tap(&mut l, Point::new(8, 8)).unwrap().gesture, Gesture::Tap);
    l.touch(TouchEvent::touch(0, Point::new(8, 8)));
    let e = l.touch(TouchEvent::touch(0, Point::new(18, 8))).unwrap();
    assert!(matches!(
        e.gesture,
        Gesture::Pan {
            phase: PanPhase::Start,
            ..
        }
    ));
    assert_eq!(e.handler.slot, 2);
    assert!(matches!(
        l.touch(TouchEvent::touch(0, Point::new(80, 8)))
            .unwrap()
            .gesture,
        Gesture::Pan {
            phase: PanPhase::Update,
            ..
        }
    ));
    let mut t = l.begin(owner).unwrap();
    t.replace_children(owner, &[]).unwrap();
    t.commit().unwrap();
    let system = l.root();
    let mut t = l.begin(system).unwrap();
    t.replace_children(system, &[]).unwrap();
    t.commit().unwrap();
    assert!(matches!(
        l.take_cancellation().unwrap().gesture,
        Gesture::Pan {
            phase: PanPhase::Cancel,
            ..
        }
    ));
    assert!(l.properties(b).is_ok());
}
// Replacing an armed target never delivers its release to new content.
#[test]
fn child_replacement_cancels_tap() {
    let (mut l, _, owner) = setup();
    let _ = button(&mut l, owner);
    l.touch(TouchEvent::touch(0, Point::new(8, 8)));
    let mut t = l.begin(owner).unwrap();
    let replacement = t.create().unwrap();
    t.set(
        replacement,
        Properties {
            bounds: rect(4, 4, 20, 20),
            bindings: Bindings {
                tap: Some(Handler {
                    owner,
                    slot: 9,
                    generation: 0,
                }),
                pan: None,
            },
            ..Properties::default()
        },
    )
    .unwrap();
    t.replace_children(owner, &[replacement]).unwrap();
    t.commit().unwrap();
    assert!(l.touch(TouchEvent::release(0, Point::new(8, 8))).is_none());
    assert_eq!(tap(&mut l, Point::new(8, 8)).unwrap().handler.slot, 9);
}
// Parent clipping affects custom graphics and input without stored child clips.
#[test]
fn inherited_clip_and_decorative_children() {
    let (mut l, _, owner) = setup();
    let b = button(&mut l, owner);
    let mut t = l.begin(owner).unwrap();
    let mut p = t.properties(owner).unwrap().clone();
    p.bounds = rect(0, 0, 12, 12);
    t.set(owner, p).unwrap();
    let mut p = t.properties(b).unwrap().clone();
    p.drawing = Drawing::Custom(3);
    t.set(b, p).unwrap();
    let decoration = t.create().unwrap();
    t.set(
        decoration,
        Properties {
            bounds: rect(0, 0, 5, 5),
            ..Properties::default()
        },
    )
    .unwrap();
    t.replace_children(b, &[decoration]).unwrap();
    t.commit().unwrap();
    assert!(tap(&mut l, Point::new(8, 8)).is_some());
    assert!(tap(&mut l, Point::new(15, 8)).is_none());
    let mut out = MockDisplay::new();
    l.render(&mut out, |_, slot, target| {
        assert_eq!(slot, 3);
        target.clear(Gray4::BLACK)
    })
    .unwrap();
    assert_eq!(out.get_pixel(Point::new(11, 11)), Some(Gray4::BLACK));
    assert_eq!(out.get_pixel(Point::new(12, 12)), None);
}
// Allocation failure cannot publish the successful edits that preceded it.
#[test]
fn capacity_is_transactional() {
    let mut l = Layers::<2>::new();
    let root = l.initialize(rect(0, 0, 20, 20)).unwrap();
    let revision = l.revision();
    let mut t = l.begin(root).unwrap();
    t.create().unwrap();
    assert_eq!(t.create(), Err(Error::Capacity));
    assert_eq!(t.commit(), Err(Error::Poisoned));
    assert_eq!(l.revision(), revision);
}

// Invalid geometry cannot replace an existing input target or advance the revision.
#[test]
fn geometry_overflow_rolls_back() {
    let (mut l, _, owner) = setup();
    let b = button(&mut l, owner);
    let revision = l.revision();
    let mut tx = l.begin(owner).unwrap();
    let mut properties = tx.properties(b).unwrap().clone();
    properties.bounds.top_left.x = i32::MAX;
    tx.set(b, properties).unwrap();
    assert_eq!(tx.commit(), Err(Error::InvalidGeometry));
    assert_eq!(l.revision(), revision);
    assert!(tap(&mut l, Point::new(8, 8)).is_some());
}

// A callback token cannot impersonate another owner, even in a system transaction.
#[test]
fn handler_owner_is_validated() {
    let (mut l, root, owner) = setup();
    let b = button(&mut l, owner);
    let mut tx = l.begin(root).unwrap();
    let mut properties = tx.properties(b).unwrap().clone();
    properties.bindings.tap.as_mut().unwrap().owner = root;
    assert_eq!(tx.set(b, properties), Err(Error::Ownership));
    assert_eq!(tx.commit(), Err(Error::Poisoned));
    assert_eq!(l.begin(b).err(), Some(Error::Ownership));
}

// Removing an outer root also revokes detached allocations of a nested owner.
#[test]
fn nested_owner_removal_revokes_detached_layers() {
    let (mut l, root, owner) = setup();
    let mut tx = l.begin(root).unwrap();
    let nested = tx.create_root().unwrap();
    let detached = tx.within(nested, |tx| tx.create()).unwrap();
    tx.replace_children(owner, &[nested]).unwrap();
    tx.commit().unwrap();
    let mut tx = l.begin(root).unwrap();
    tx.destroy(owner).unwrap();
    tx.commit().unwrap();
    assert_eq!(l.properties(nested), Err(Error::StaleHandle));
    assert_eq!(l.properties(detached), Err(Error::StaleHandle));
}
