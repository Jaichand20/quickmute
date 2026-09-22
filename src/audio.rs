use windows::core::{Result, GUID};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};

pub struct AudioManager {
    volume: IAudioEndpointVolume,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let volume = Self::init_endpoint()?;
            Ok(Self { volume })
        }
    }

    fn init_endpoint() -> Result<IAudioEndpointVolume> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            // Try default console recording device first, fall back to communications device
            let device = enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .or_else(|_| enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications))?;

            device.Activate(CLSCTX_ALL, None)
        }
    }

    fn refresh(&mut self) -> Result<()> {
        self.volume = Self::init_endpoint()?;
        Ok(())
    }

    pub fn is_muted(&mut self) -> Result<bool> {
        unsafe {
            match self.volume.GetMute() {
                Ok(muted) => Ok(muted.as_bool()),
                Err(_) => {
                    self.refresh()?;
                    let muted = self.volume.GetMute()?;
                    Ok(muted.as_bool())
                }
            }
        }
    }

    pub fn set_mute(&mut self, mute: bool) -> Result<()> {
        unsafe {
            let b_mute = windows::Win32::Foundation::BOOL::from(mute);
            match self.volume.SetMute(b_mute, std::ptr::null_mut() as *const GUID) {
                Ok(_) => Ok(()),
                Err(_) => {
                    self.refresh()?;
                    self.volume.SetMute(b_mute, std::ptr::null_mut() as *const GUID)?;
                    Ok(())
                }
            }
        }
    }

    pub fn toggle_mute(&mut self) -> Result<bool> {
        let current = self.is_muted()?;
        let next = !current;
        self.set_mute(next)?;
        Ok(next)
    }
}
