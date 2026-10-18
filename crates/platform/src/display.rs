//! Common frame submission for display consumers.

use crate::Display;

/// This function renders and submits a requested frame when the display is ready.
/// True confirms submission, not physical completion.
/// The caller keeps its redraw request after an error or a false result.
pub fn present<D: Display>(
    display: &mut D,
    requested: bool,
    render: impl FnOnce(&mut D) -> Result<(), D::Error>,
) -> Result<bool, D::Error> {
    if !requested || display.is_busy() {
        return Ok(false);
    }
    render(display)?;
    display.refresh()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::{pixelcolor::Gray4, prelude::*};

    #[derive(Default)]
    struct Target {
        busy: bool,
        fail_refresh: bool,
        submissions: u32,
    }

    impl OriginDimensions for Target {
        fn size(&self) -> Size {
            Size::new(4, 3)
        }
    }

    impl DrawTarget for Target {
        type Color = Gray4;
        type Error = &'static str;

        fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
            &mut self,
            _: I,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Display for Target {
        fn is_busy(&self) -> bool {
            self.busy
        }

        fn refresh(&mut self) -> Result<(), Self::Error> {
            if self.fail_refresh {
                return Err("refresh");
            }
            self.submissions += 1;
            self.busy = true;
            Ok(())
        }
    }

    #[test]
    fn absent_requests_and_busy_displays_do_not_render() {
        let mut target = Target::default();
        assert_eq!(
            present(&mut target, false, |_| panic!("unexpected render")),
            Ok(false)
        );
        target.busy = true;
        assert_eq!(
            present(&mut target, true, |_| panic!("unexpected render")),
            Ok(false)
        );
        assert_eq!(target.submissions, 0);
    }

    #[test]
    fn errors_do_not_report_submission_and_allow_a_retry() {
        let mut target = Target::default();
        assert_eq!(present(&mut target, true, |_| Err("draw")), Err("draw"));
        assert_eq!(target.submissions, 0);
        target.fail_refresh = true;
        assert_eq!(present(&mut target, true, |_| Ok(())), Err("refresh"));
        assert_eq!(target.submissions, 0);
        target.fail_refresh = false;
        assert_eq!(present(&mut target, true, |_| Ok(())), Ok(true));
        assert_eq!(target.submissions, 1);
        assert!(target.busy);
    }
}
