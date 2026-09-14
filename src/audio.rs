use windows::core::{Result, GUID};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

pub struct AudioManager {
    volume: IAudioEndpointVolume,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let device = enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications)?;
            let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

            Ok(Self { volume })
        }
    }

    pub fn is_muted(&self) -> Result<bool> {
        unsafe {
            let muted = self.volume.GetMute()?;
            Ok(muted.as_bool())
        }
    }

    pub fn set_mute(&self, mute: bool) -> Result<()> {
        unsafe {
            let b_mute = windows::Win32::Foundation::BOOL::from(mute);
            self.volume.SetMute(b_mute, std::ptr::null_mut() as *const GUID)?;
            Ok(())
        }
    }

    pub fn toggle_mute(&self) -> Result<bool> {
        let current = self.is_muted()?;
        let next = !current;
        self.set_mute(next)?;
        Ok(next)
    }
}
