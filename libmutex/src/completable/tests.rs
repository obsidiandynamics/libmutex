// use std::sync::{Arc, Barrier};
// use std::thread;
// use crate::completable::{Completable};
// use crate::test_utils::SHORT_WAIT;
//
// #[test]
// #[ignore]
// fn complete_later() {
//     let comp = Completable::default();
//     assert!(!comp.is_complete());
//
//     assert!(comp.complete(42));
//     assert!(comp.is_complete());
//     assert_eq!(42, *comp.get());
//     assert_eq!(Some(42), *comp.peek());
//     assert_eq!(Some(42), *comp.try_get(SHORT_WAIT));
//
//     // assigning a different value should not overwrite the existing
//     assert!(!comp.complete(69));
//     assert!(comp.is_complete());
//     assert_eq!(Some(42), *comp.peek());
//
//     assert_eq!(Some(42), comp.into_inner())
// }
//
// #[test]
// #[ignore]
// fn complete_at_init() {
//     let comp = Completable::new(42);
//     assert!(comp.is_complete());
//     assert_eq!(42, *comp.get());
//     assert_eq!(Some(42), *comp.peek());
//     assert_eq!(Some(42), *comp.try_get(SHORT_WAIT));
//
//     // assigning a different value should not overwrite the existing
//     assert!(!comp.complete(69));
//     assert!(comp.is_complete());
//     assert_eq!(Some(42), *comp.peek());
//
//     assert_eq!(Some(42), comp.into_inner())
// }
//
// #[test]
// #[ignore]
// fn await_complete() {
//     let comp = Arc::new(Completable::default());
//
//     let t_2_should_complete = Arc::new(Barrier::new(2));
//     let t_2 = {
//         let comp = comp.clone();
//         let t_2_should_complete = t_2_should_complete.clone();
//         thread::spawn(move || {
//             t_2_should_complete.wait();
//             assert!(comp.complete(42));
//             assert!(!comp.complete(69));
//             assert_eq!(42, *comp.get());
//         })
//     };
//
//     assert_eq!(None, *comp.try_get(SHORT_WAIT));
//     t_2_should_complete.wait();
//
//     assert_eq!(42, *comp.get());
//     assert!(comp.is_complete());
//     assert_eq!(Some(42), *comp.peek());
//     t_2.join().unwrap();
// }
//
// #[test]
// #[ignore]
// fn complete_exclusive() {
//     let comp = Completable::default();
//
//     let mut invoked = false;
//     comp.complete_exclusive(|| {
//         invoked = true;
//         42
//     });
//     assert_eq!(42, *comp.get());
//     assert!(invoked);
//
//     invoked = false;
//     comp.complete_exclusive(|| {
//         invoked = true;
//         69
//     });
//     assert_eq!(42, *comp.get());
//     assert!(!invoked);
// }
//
// #[test]
// #[ignore]
// fn completable_is_sync() {
//     fn sync<T: Sync>(_: T) {}
//
//     let comp = Completable::new(());
//     sync(comp.peek());
//     sync(comp.get());
//     sync(comp);
// }
