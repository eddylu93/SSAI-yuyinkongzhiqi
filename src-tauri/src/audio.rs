use cpal::traits::{DeviceTrait,HostTrait,StreamTrait};
use cpal::{Sample,SizedSample,FromSample};
use std::sync::{Arc,Mutex};
use base64::Engine;

pub fn names()->Result<Vec<String>,String>{
    cpal::default_host().input_devices().map_err(|e|e.to_string()).map(|devices|devices.filter_map(|d|d.name().ok()).collect())
}
pub fn record(name:String)->Result<String,String>{
    let wav=capture(name,std::time::Duration::from_secs(3),|duration|{std::thread::sleep(duration);true})?;
    Ok(format!("data:audio/wav;base64,{}",base64::engine::general_purpose::STANDARD.encode(wav)))
}
pub fn capture(name:String,duration:std::time::Duration,wait:impl FnOnce(std::time::Duration)->bool)->Result<Vec<u8>,String>{
    let list:Vec<_>=cpal::default_host().input_devices().map_err(|e|e.to_string())?.filter(|d|d.name().ok().as_deref()==Some(&name)).collect();
    if list.len()!=1{return Err("麦克风标识不唯一或设备已断开，请重新选择".into())}
    let dev=&list[0];let cfg=dev.default_input_config().map_err(|e|e.to_string())?;
    let samples=Arc::new(Mutex::new(Vec::<i16>::new()));let failed=Arc::new(Mutex::new(None));
    fn stream<T:SizedSample+Sample>(dev:&cpal::Device,cfg:&cpal::SupportedStreamConfig,out:Arc<Mutex<Vec<i16>>>,error:Arc<Mutex<Option<String>>>,duration:std::time::Duration)->Result<cpal::Stream,String> where i16:FromSample<T>{
        let channels=cfg.channels() as usize;let max=cfg.sample_rate().0 as usize*duration.as_secs() as usize;
        dev.build_input_stream(&cfg.clone().into(),move|block:&[T],_|{
            if let Ok(mut buf)=out.lock(){let left=max.saturating_sub(buf.len());buf.extend(block.chunks_exact(channels).take(left).map(|f|i16::from_sample(f[0])))}
        },move|e|*error.lock().unwrap()=Some(e.to_string()),None).map_err(|e|e.to_string())
    }
    let input=match cfg.sample_format(){
        cpal::SampleFormat::F32=>stream::<f32>(dev,&cfg,samples.clone(),failed.clone(),duration),
        cpal::SampleFormat::I16=>stream::<i16>(dev,&cfg,samples.clone(),failed.clone(),duration),
        cpal::SampleFormat::I32=>stream::<i32>(dev,&cfg,samples.clone(),failed.clone(),duration),
        cpal::SampleFormat::U16=>stream::<u16>(dev,&cfg,samples.clone(),failed.clone(),duration),
        _=>return Err("当前录音格式暂不支持".into())
    }?;
    input.play().map_err(|e|e.to_string())?;let keep=wait(duration);drop(input);
    if !keep{return Err("录音已取消或达到时长上限".into())}
    if let Some(e)=failed.lock().unwrap().take(){return Err(e)}
    let data=samples.lock().unwrap();if data.is_empty(){return Err("没有收到音频采样".into())}
    let rate=cfg.sample_rate().0;let n=data.len() as u32*2;let mut wav=Vec::new();
    wav.extend(b"RIFF");wav.extend((36+n).to_le_bytes());wav.extend(b"WAVEfmt ");wav.extend(16u32.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(rate.to_le_bytes());wav.extend((rate*2).to_le_bytes());wav.extend(2u16.to_le_bytes());wav.extend(16u16.to_le_bytes());wav.extend(b"data");wav.extend(n.to_le_bytes());for value in data.iter(){wav.extend(value.to_le_bytes())}
    Ok(wav)
}
