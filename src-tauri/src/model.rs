use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap,BTreeSet};

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="camelCase",deny_unknown_fields)]
pub enum Action {
    None, FocusCodex, FocusChat{target:String}, Shortcut{keys:Vec<String>}, Scroll{steps:i32}, Text{phrase:String},
    Launch{app:String,#[serde(default,skip_serializing_if="Option::is_none")] name:Option<String>}, Sequence{id:String}, Wait{ms:u64},
}
pub const SLOTS:[&str;13]=["K2.single","K2.double","K2.long","K3.single","K3.double","K3.long","DIAL.single","DIAL.double","DIAL.long","DIAL.left","DIAL.right","DIAL.heldLeft","DIAL.heldRight"];
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Profile {pub id:String,pub name:String,pub apps:Vec<String>,pub bindings:BTreeMap<String,Action>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sequence {pub name:String,pub steps:Vec<Action>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Config{
    #[serde(default = "previous_installation")] pub setup_complete:bool,
    #[serde(default)] pub vendor_controls:bool,
    pub schema_version:u32,pub revision:u64,pub enabled:bool,pub voice_enabled:bool,
    pub tool:String,pub voice_keys:Vec<String>,pub toggle_voice:bool,pub endpoint:String,
    pub follow:bool,pub active:String,pub profiles:Vec<Profile>,pub sequences:BTreeMap<String,Sequence>,
    pub phrases:BTreeMap<String,String>,pub history:bool,pub agent:bool,pub theme:String,
}
fn previous_installation()->bool{true}
impl Default for Config{
    fn default()->Self{
        let mut bindings=BTreeMap::new();
        for (slot,key) in [("K2.single","Enter"),("K3.single","Esc"),("DIAL.single","Backspace")]{
            bindings.insert(slot.into(),Action::Shortcut{keys:vec![key.into()]});
        }
        if cfg!(windows){bindings.insert("DIAL.double".into(),Action::FocusCodex);}
        bindings.insert("DIAL.left".into(),Action::Scroll{steps:1});
        bindings.insert("DIAL.right".into(),Action::Scroll{steps:-1});
        Self{setup_complete:false,vendor_controls:false,schema_version:1,revision:0,enabled:true,voice_enabled:false,tool:if cfg!(windows){"Windows 系统识别"}else{"自定义"}.into(),voice_keys:if cfg!(windows){vec!["Ctrl".into(),"Win".into()]}else{vec!["RightOption".into()]},toggle_voice:false,endpoint:String::new(),follow:false,active:"default".into(),profiles:vec![Profile{id:"default".into(),name:"默认".into(),apps:vec![],bindings}],sequences:BTreeMap::new(),phrases:BTreeMap::new(),history:false,agent:false,theme:"dark".into()}
    }
}
pub fn vk(key:&str)->Option<u16>{
    let value=match key{
        "Ctrl"=>0x11,"LeftCtrl"=>0xa2,"RightCtrl"=>0xa3,"Alt"|"Option"=>0x12,"LeftAlt"|"LeftOption"=>0xa4,"RightAlt"|"RightOption"=>0xa5,
        "Shift"=>0x10,"LeftShift"=>0xa0,"RightShift"=>0xa1,"Win"|"Cmd"|"LeftCmd"=>0x5b,"RightWin"|"RightCmd"=>0x5c,
        "Enter"=>13,"Esc"=>27,"Tab"=>9,"Backspace"=>8,"Space"=>32,"Delete"=>46,
        "Left"=>37,"Up"=>38,"Right"=>39,"Down"=>40,"Home"=>36,"End"=>35,"PageUp"=>33,"PageDown"=>34,
        "VolumeUp"=>175,"VolumeDown"=>174,"Mute"=>173,"PlayPause"=>179,
        s if s.len()==1 && s.as_bytes()[0].is_ascii_alphanumeric()=>s.as_bytes()[0].to_ascii_uppercase() as u16,
        s if s.starts_with('F')=>{let n=s[1..].parse::<u16>().ok()?;if !(1..=24).contains(&n){return None}0x6f+n},
        _=>return None
    };Some(value)
}
fn validate_keys(keys:&[String])->bool{
    !keys.is_empty() && keys.len()<=5 && keys.iter().all(|k|vk(k).is_some()) &&
        keys.iter().filter_map(|k|vk(k)).collect::<BTreeSet<_>>().len()==keys.len()
}
impl Config {
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=1 || self.profiles.is_empty() || self.profiles.len()>64 || !validate_keys(&self.voice_keys)
            || !["dark","light","system"].contains(&self.theme.as_str()){return Err("配置版本、主题或语音快捷键无效".into())}
        if self.profiles.iter().map(|p|&p.id).collect::<BTreeSet<_>>().len()!=self.profiles.len()
            || !self.profiles.iter().any(|p|p.id==self.active) || !self.profiles.iter().any(|p|p.id=="default"){return Err("配置标识冲突或缺少默认配置".into())}
        let mut apps=BTreeSet::new();
        for p in &self.profiles{
            if p.id.is_empty() || p.name.trim().is_empty() || p.apps.iter().any(|a|a.is_empty() || !apps.insert(a.to_lowercase())) {return Err("配置名称或应用关联无效".into())}
            for(slot,a)in &p.bindings{if !SLOTS.contains(&slot.as_str()){return Err("未知控件手势".into())}self.validate_action(a,false)?;}
        }
        for seq in self.sequences.values(){
            if seq.steps.is_empty() || seq.steps.len()>20 || seq.name.trim().is_empty(){return Err("组合应包含 1–20 步及名称".into())}
            for a in &seq.steps{self.validate_action(a,true)?;}
        }
        Ok(())
    }
    fn validate_action(&self,a:&Action,in_sequence:bool)->Result<(),String>{
        let valid=match a{
            Action::None|Action::FocusCodex=>true,Action::FocusChat{target}=>crate::composer::target(target).is_some(),Action::Shortcut{keys}=>validate_keys(keys),Action::Scroll{steps}=>*steps!=0 && (-20..=20).contains(steps),
            Action::Text{phrase}=>self.phrases.contains_key(phrase),
            Action::Launch{app,..}=>crate::platform::valid_launch_target(app),
            Action::Sequence{id}=>!in_sequence && self.sequences.contains_key(id),
            Action::Wait{ms}=>in_sequence && *ms<=10000,
        };if valid{Ok(())}else{Err("动作参数、组合引用或短语引用无效".into())}
    }
    pub fn profile(&self,foreground:&str)->&Profile{
        let id=if self.follow{self.profiles.iter().find(|p|p.apps.iter().any(|a|a.eq_ignore_ascii_case(foreground))).map(|p|p.id.as_str()).unwrap_or("default")}else{&self.active};
        self.profiles.iter().find(|p|p.id==id).unwrap_or(&self.profiles[0])
    }
}
