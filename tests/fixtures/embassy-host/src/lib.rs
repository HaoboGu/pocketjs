use pocket_runtime::{Cow, InputClasses, Profile, RuntimeConfig};
pub fn config(viewport: [u32; 2]) -> RuntimeConfig {
    RuntimeConfig {
        profile_hash: [0x5a; 32],
        host_abi: pocket_drivers::spec::BINDING_GENERATION,
        heap_limit: 1024 * 1024,
        frame_buffer_budget: viewport[0] as usize * viewport[1] as usize * 2,
        profile: Profile {
            id: Cow::Borrowed("embassy-test"),
            cadences: Cow::Borrowed(&[60]),
            viewport,
            logical_viewports: Cow::Owned(vec![viewport]),
            dynamic_viewport: None,
            presentations: Cow::Borrowed(&[Cow::Borrowed("native")]),
            auxiliary: None,
            auxiliary_logical: Cow::Borrowed(&[]),
            density: 1,
            animation: Cow::Borrowed("composited"),
            capabilities: Cow::Borrowed(&[]),
            required_inputs: InputClasses::NONE,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pocket_drivers::{DamageRect, InputFrameBuilder};
    use pocket_embassy::{Display, Error, QuickJs, Ui};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    #[test]
    fn realm_owns_assets_and_reports_execution_errors() {
        let mut guest = QuickJs::new(
            config([16, 16]),
            128 * 1024,
            b"globalThis.frame=()=>{}",
            Cow::Owned(vec![7, 8]),
        )
        .unwrap();
        guest.evaluate(b"if(new Uint8Array(__pak)[0]!==7)throw Error('assets'); Promise.resolve(42).then(v=>globalThis.result=v)").unwrap();
        guest
            .evaluate(b"if(result!==42)throw Error('jobs')")
            .unwrap();
        for source in [
            b"throw Error('probe')".as_slice(),
            b"while(true){}",
            b"new ArrayBuffer(2*1024*1024)",
            b"new Uint8Array(__pak).reverse()",
        ] {
            assert!(
                matches!(guest.evaluate(source), Err(Error::JavaScript(_))),
                "{}",
                String::from_utf8_lossy(source)
            );
        }
        guest
            .evaluate(b"if(new Uint8Array(__pak)[0]!==7)throw Error('mutated')")
            .unwrap();
    }
    #[test]
    fn display_yields_and_cancelled_transfer_repaints() {
        struct Screen {
            pending: bool,
            areas: Vec<DamageRect>,
        }
        impl Display for Screen {
            type Error = ();
            async fn present(
                &mut self,
                pixels: &[u16],
                width: u32,
                regions: &[DamageRect],
            ) -> Result<(), ()> {
                assert_eq!(width, 24);
                assert_eq!(pixels.len(), 24 * 16);
                self.areas = regions.to_vec();
                if self.pending {
                    std::future::pending::<()>().await;
                }
                Ok(())
            }
        }
        let guest = QuickJs::new(
            config([24, 16]),
            128 * 1024,
            b"globalThis.frame=(buttons)=>{globalThis.buttons=buttons}",
            Cow::Borrowed(&[]),
        )
        .unwrap();
        let mut ui = Ui::new(guest).unwrap();
        let mut input = InputFrameBuilder::new();
        input.buttons = 5;
        let mut screen = Screen {
            pending: true,
            areas: vec![],
        };
        let mut context = Context::from_waker(Waker::noop());
        {
            let frame = input.frame();
            let mut future = std::pin::pin!(ui.frame(&frame, &mut screen));
            assert!(future.as_mut().poll(&mut context).is_pending());
        }
        screen.pending = false;
        {
            let frame = input.frame();
            let mut future = std::pin::pin!(ui.frame(&frame, &mut screen));
            assert!(matches!(
                future.as_mut().poll(&mut context),
                Poll::Ready(Ok(()))
            ));
        }
        assert_eq!(screen.areas.len(), 1);
        assert_eq!(screen.areas[0], DamageRect::new(0, 0, 24, 16));
        ui.guest()
            .evaluate(b"if(buttons!==5)throw Error('input')")
            .unwrap();
    }
    #[test]
    fn framebuffer_budget_is_checked() {
        let mut config = config([16, 16]);
        config.frame_buffer_budget = 1;
        let guest = QuickJs::new(config, 128 * 1024, b"", Cow::Borrowed(&[])).unwrap();
        assert!(Ui::new(guest).is_err());
    }
}
