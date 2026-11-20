use crate::{model::{Action,Config},platform};
use rusqlite::{params,Connection};
use serde_json::{json,Value};
use std::{collections::{HashMap,VecDeque},path::Path,sync::{Arc,Mutex},time::{Duration,Instant}};

pub type Shared=Arc<Mutex<Runtime>>;
struct Held { at:Instant, long:Action, single:Action, double:Action, consumed:bool, is_second:bool,repeat:Option<Instant> }
struct Pending {due:Instant,action:Action}
struct Voice {instance:String,at:Instant,owned:Vec<u16>,toggle:bool,keys:Vec<String>,capture:Option<crate::speech::Capture>,target:String,record:bool}
struct ResultJob{rx:std::sync::mpsc::Receiver<Result<(String,String),String>>,target:String,record:bool,verify:bool}
struct Run { steps:VecDeque<Action>,due:Instant,target:String,awaiting:Option<(String,Instant)>, completed:usize }
pub struct Runtime {
    input_test: crate::input_test::InputTest,
    focus: Option<crate::composer::FocusJob>,
    launcher: platform::Launcher,
    pub direct:crate::direct::Direct,pub config:Config,pub db:Connection,pub online:bool,pub bindings:HashMap<String,String>,
    pub event_count:u64,pub message:String,pub diagnostic:bool,pub recent:VecDeque<String>,
    held:HashMap<String,Held>,pending:HashMap<String,Pending>,voice:Option<Voice>,run:Option<Run>,result:Option<ResultJob>,
}
impl Runtime {
    pub fn open(path:&Path)->Result<Self,String>{
        std::fs::create_dir_all(path).map_err(|e|e.to_string())?;
        let db=Connection::open(path.join("desk.sqlite3")).map_err(|e|e.to_string())?;
        let version:i64=db.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if version>1{return Err("数据库来自更新版本，请升级 SSAI-语音控制器".into())}
        let existing:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='usage')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if version==0 && existing{
            let backup=path.join(format!("before-v1-{}.sqlite3",uuid::Uuid::new_v4()));
            db.execute("VACUUM INTO ?1",[backup.to_string_lossy().as_ref()]).map_err(|e|e.to_string())?;
        }
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS usage(id INTEGER PRIMARY KEY, stamp INTEGER NOT NULL, control TEXT NOT NULL, duration INTEGER, interrupted INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS history(id TEXT PRIMARY KEY, app TEXT NOT NULL, stamp INTEGER NOT NULL, text TEXT NOT NULL);").map_err(|e|e.to_string())?;
        if version==0{db.execute_batch("BEGIN IMMEDIATE; ALTER TABLE usage ADD COLUMN end_ms INTEGER; CREATE INDEX IF NOT EXISTS usage_by_stamp ON usage(stamp); PRAGMA user_version=1; COMMIT;").map_err(|e|e.to_string())?;}
        use rusqlite::OptionalExtension;
        let saved:Option<String>=db.query_row("SELECT body FROM settings WHERE id=1",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        let config:Config=match saved{Some(s)=>serde_json::from_str(&s).map_err(|_|"配置损坏，请恢复备份")?,None=>Config::default()};
        config.validate()?;
        let direct=crate::direct::Direct::load(&db)?;
        Ok(Self{launcher:Default::default(),focus:None,input_test:Default::default(),direct,config,db,online:false,bindings:HashMap::new(),event_count:0,message:"正在检测 AU05 接收器".into(),diagnostic:false,recent:VecDeque::new(),held:HashMap::new(),pending:HashMap::new(),voice:None,run:None,result:None})
    }
    pub fn save(&mut self,mut config:Config)->Result<(),String>{
        config.validate()?;
        if config.revision!=self.config.revision{return Err("配置已更新，请重新加载".into())}
        config.revision+=1;
        self.db.execute("INSERT OR REPLACE INTO settings VALUES(1,?1)",[serde_json::to_string(&config).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        if self.config.enabled && !config.enabled{self.held.clear();self.pending.clear();self.cancel()}
        if self.config.voice_enabled && !config.voice_enabled{self.end_voice(true)}
        if !config.history {if let Some(v)=self.voice.as_mut(){v.record=false}if let Some(r)=self.result.as_mut(){r.record=false}}
        if self.config.vendor_controls!=config.vendor_controls {self.set_diagnostic(self.diagnostic);}
        self.config=config;Ok(())
    }
    pub fn snapshot(&self)->Value{
        json!({"direct":self.direct.snapshot(),"online":self.online,"bindings":self.bindings,"events":self.event_count,"message":self.message,
           "diagnostic":self.diagnostic,"inputTest":self.input_test,"recent":self.recent,"speaking":self.voice.is_some(),"running":self.run.is_some(),"revision":self.config.revision})
    }
    fn log(&mut self,msg:String){ self.message=msg.clone();self.recent.push_front(msg);self.recent.truncate(24); }
    fn count(&mut self,control:&str,duration:Option<i64>,interrupted:bool){
        if let Err(e)=self.db.execute("INSERT INTO usage(stamp,control,duration,interrupted,end_ms) VALUES(unixepoch(),?1,?2,?3,?4)",params![control,duration,interrupted,chrono::Utc::now().timestamp_millis()]){
            self.message=format!("统计写入失败：{e}");
        }
    }
    pub fn cleanup(&mut self){
        self.input_test.interrupt();
        self.result=None;
        self.end_voice(true);self.held.clear();self.pending.clear();self.cancel();self.bindings.clear();self.online=false;
        self.log("AU05 输入已断开".into());
    }
    pub fn set_diagnostic(&mut self,value:bool){
        if value && !self.diagnostic { self.input_test=Default::default(); }
        self.input_test.interrupt();
        self.direct.reset();
        self.result=None;
        self.end_voice(true);self.held.clear();self.pending.clear();self.cancel();self.diagnostic=value;
    }
    fn end_voice(&mut self,interrupted:bool){
        if let Some(mut v)=self.voice.take(){
            platform::release(&mut v.owned);
            let mut msg=if interrupted{"语音已中断，请检查输入工具"}else{"已发送结束指令，等待输入工具出字"}.to_string();
            if v.toggle && !interrupted{if let Err(e)=platform::tap(&v.keys){msg=e}}
            if let Some(cap)=v.capture.take(){
                cap.finish(!interrupted);
                if !interrupted{
                    let(tx,rx)=std::sync::mpsc::channel();let app=v.target.clone();
                    std::thread::spawn(move||{
                        let result=cap.output.recv().map_err(|_|"录音中断".to_owned()).and_then(|r|r).and_then(|(wav,id)|{
                            let text=crate::speech::recognize(&wav)?;
                            if platform::foreground()!=app{return Err("目标窗口已切换".into())}
                            crate::speech::target(&id,"")?;Ok((text,id))
                        });let _=tx.send(result);
                    });
                    self.result=Some(ResultJob{rx,target:v.target,record:v.record,verify:false});
                    msg="正在使用 Windows 本地识别器".into();
                }
            }
            self.count("K1",Some(v.at.elapsed().as_millis() as i64),interrupted);self.log(msg);
        }
    }
    pub fn input(&mut self,id:&str,control:&str,phase:&str,steps:i32)->Result<(),String>{
        if self.bindings.get(id).map(String::as_str)!=Some(control){return Err("控件实例未绑定".into())}
        self.event_count+=1;
        if matches!(phase, "turn" | "heldTurn") {
            let direction=if steps>0{"右旋"}else{"左旋"};
            self.log(format!("{control} · {phase} · {direction} {} 格",steps.unsigned_abs()));
        } else { self.log(format!("{control} · {phase}")); }
        if self.diagnostic { self.input_test.record(control,phase,steps); return Ok(()); }
        if control=="K1"{
            if phase=="up" && self.voice.as_ref().is_some_and(|v|v.instance==id){self.end_voice(false)}
            if phase=="down" && self.voice.is_none(){
                if !self.config.voice_enabled{return Ok(())}
                if self.focus.is_some(){return Err("正在聚焦聊天框，请等待就绪再按住语音键".into())}
                let keys=self.config.voice_keys.clone();
                self.result=None;
                let local=self.config.tool=="Windows 系统识别";
                if local && self.config.endpoint.is_empty(){return Err("请先选择 AU05 麦克风".into())}
                let capture=local.then(||crate::speech::Capture::start(self.config.endpoint.clone()));
                let toggle=!local && self.config.toggle_voice;
                let owned=if local{Vec::new()}else if toggle{platform::tap(&keys)?;Vec::new()}else{platform::press(&keys)?};
                self.voice=Some(Voice{instance:id.into(),at:Instant::now(),owned,toggle,keys,capture,target:platform::foreground(),record:self.config.history});
                self.log("已触发输入工具".into());
            }
            return Ok(())
        }
        if !self.config.enabled{return Ok(())}
        let fg=platform::foreground();let profile=self.config.profile(&fg);
        let action=|gesture:&str|profile.bindings.get(&format!("{control}.{gesture}")).cloned().unwrap_or(Action::None);
        match phase{
            "turn"|"heldTurn"=>{
                let gesture=match(phase,steps>0){("turn",true)=>"right",("turn",false)=>"left",(_,true)=>"heldRight",_=>"heldLeft"};
                let a=action(gesture);
                if let Some(h)=self.held.get_mut(id){h.consumed=true}
                for _ in 0..steps.unsigned_abs(){self.count("DIAL.turn",None,false);self.execute(a.clone())?;}
            }
            "down" if !self.held.contains_key(id)=>{
                let (single,double,long)=(action("single"),action("double"),action("long"));
                let second=self.pending.remove(id).is_some();
                let immediate=control!="DIAL" && matches!(double,Action::None)&&matches!(long,Action::None);
                let repeats=immediate && match &single{Action::Scroll{..}=>true,Action::Shortcut{keys}=>keys.len()==1 && ["Left","Right","Up","Down","PageUp","PageDown","Backspace"].contains(&keys[0].as_str()),_=>false};
                self.held.insert(id.into(),Held{at:Instant::now(),long,single:single.clone(),double,consumed:immediate,is_second:second,repeat:repeats.then(||Instant::now()+Duration::from_millis(450))});
                self.count(control,None,false);if immediate{self.execute(single)?}
            }
            "up"=>if let Some(h)=self.held.remove(id){
                if !h.consumed{
                    if h.is_second{self.execute(h.double)?}
                    else if matches!(h.double,Action::None){self.execute(h.single)?}
                    else{self.pending.insert(id.into(),Pending{due:Instant::now()+Duration::from_millis(300),action:h.single});}
                }
            },_=>{}
        }
        Ok(())
    }
    fn execute(&mut self,a:Action)->Result<(),String>{
        match a{
            Action::None=>{},Action::Shortcut{keys}=>platform::tap(&keys)?,Action::Scroll{steps}=>platform::scroll(steps)?,
            Action::Text{phrase}=>platform::text(self.config.phrases.get(&phrase).ok_or("常用语不存在")?)?,
            Action::FocusCodex=>self.start_focus("codex")?,
            Action::FocusChat{target}=>self.start_focus(&target)?,
            Action::Launch{app,..}=>{self.launcher.request(&app)?;},
            Action::Sequence{id}=>{if self.run.is_some(){return Err("组合正在执行".into())}
                let seq=self.config.sequences.get(&id).ok_or("组合不存在")?;
                self.run=Some(Run{steps:seq.steps.clone().into(),due:Instant::now(),target:platform::foreground(),awaiting:None,completed:0});
            },Action::Wait{..}=>return Err("等待仅适用于组合".into())
        }
        Ok(())
    }
    pub fn cancel(&mut self){self.launcher.cancel();self.focus=None;self.run=None}
    fn start_focus(&mut self,id:&str)->Result<(),String>{
        if self.focus.is_some() || self.voice.is_some(){return Err("聚焦或语音操作正在进行".into())}
        let target=crate::composer::target(id).ok_or("未知聊天工具")?;
        self.focus=Some(crate::composer::FocusJob::start(target)?);
        self.log(format!("正在聚焦 {} 聊天框",target.name));
        Ok(())
    }
    pub fn try_focus(&mut self,target:&str)->Result<(),String>{
        if self.run.is_some(){return Err("组合正在运行".into())}
        self.start_focus(target)
    }
    pub fn try_sequence(&mut self,id:String)->Result<(),String>{self.execute(Action::Sequence{id})}
    pub fn tick(&mut self){
        if self.voice.as_ref().is_some_and(|v|v.at.elapsed()>=Duration::from_secs(120)){
            self.end_voice(true);
            self.message="语音达到两分钟上限，已结束本次会话".into();
        }
        self.held.retain(|_,h|h.at.elapsed()<Duration::from_secs(120));
        if self.direct.expire(){self.message="校准超时，请重新开始".into();}
        self.poll_result();
        let now=Instant::now();let mut queue=Vec::new();
        for h in self.held.values_mut(){
            if !h.consumed && !matches!(h.long,Action::None) && now.duration_since(h.at)>=Duration::from_millis(550){h.consumed=true;queue.push(h.long.clone())}
            if h.repeat.is_some_and(|due|now>=due){queue.push(h.single.clone());h.repeat=Some(now+Duration::from_millis(85))}
        }
        self.pending.retain(|_,p|{if now>=p.due{queue.push(p.action.clone());false}else{true}});
        for a in queue{if let Err(e)=self.execute(a){self.log(e)}}
        if let Some((binding,error))=self.launcher.poll(){
            if let Some(run)=self.run.as_ref().filter(|r|r.awaiting.as_ref().is_some_and(|(target,_)|*target==binding)){
                let step=run.completed;
                self.run=None;
                self.log(format!("第 {step} 步失败：{error}"));
            }else{self.log(error)}
        }
        if let Some(job)=self.focus.as_mut(){
            let Some(result)=job.poll() else{return};
            let name=job.target.name.clone();
            self.focus=None;
            let foreground=platform::foreground();
            if let Err(e)=result{self.run=None;self.log(e);return}
            if let Some(run)=self.run.as_mut(){run.target=foreground;}
            self.log(format!("{name} 输入框已聚焦，可以按住语音键"));
        }
        if self.run.as_ref().is_some_and(|r|r.due<=now){
            let mut run=self.run.take().unwrap();
            let foreground=platform::foreground();
            if let Some((target,until))=&run.awaiting{
                if self.launcher.waiting_for(target){
                    run.due=now+Duration::from_millis(100);self.run=Some(run);return
                }
                if !platform::launch_is_foreground(target,&foreground){
                    if now>=*until{self.log(format!("第 {} 步超时：目标应用未激活",run.completed));return}
                    run.due=now+Duration::from_millis(100);self.run=Some(run);return
                }
                run.target=foreground.clone();run.awaiting=None;
            }
            if run.target!=foreground{self.log(format!("第 {} 步已停止：前台应用改变",run.completed+1));return}
            if let Some(a)=run.steps.pop_front(){
                run.completed+=1;
                match a{
                    Action::Wait{ms}=>run.due=now+Duration::from_millis(ms),
                    Action::Launch{app,..}=>match self.launcher.request(&app){
                        Ok(identity)=>run.awaiting=Some((identity,now+Duration::from_secs(8))),
                        Err(e)=>{self.log(format!("第 {} 步失败：{e}",run.completed));return}
                    },
                    _=>if let Err(e)=self.execute(a){self.log(format!("第 {} 步失败：{e}",run.completed));return}
                }
            }
            if !run.steps.is_empty() || run.awaiting.is_some() || self.focus.is_some(){self.run=Some(run)}else{self.log("组合执行完毕".into())}
        }
    }
    fn poll_result(&mut self){
        let Some(job)=self.result.take()else{return};
        let candidate=match job.rx.try_recv(){
            Ok(Ok(value))=>value,Ok(Err(e))=>{self.log(e);return},
            Err(std::sync::mpsc::TryRecvError::Empty)=>{self.result=Some(job);return},
            Err(_)=>{self.log("识别任务中断".into());return}
        };
        let (text,id)=candidate;
        if platform::foreground()!=job.target{self.log("目标已切换，已丢弃本次结果".into());return}
        if job.verify{
            if job.record && self.config.history{
                let saved=self.db.execute("INSERT INTO history(id,app,stamp,text) VALUES(?1,?2,unixepoch(),?3)",params![uuid::Uuid::new_v4().to_string(),job.target,text]);
                if saved.is_err(){self.log("已出字，回眸保存失败".into());return}
            }
            self.log("本次文本已确认".into());return
        }
        if let Err(e)=platform::text(&text){self.log(e);return}
        if !job.record || !self.config.history{self.log("已提交识别文本".into());return}
        let(tx,rx)=std::sync::mpsc::channel();
        std::thread::spawn(move||{let ok=crate::speech::target(&id,&text).map(|_|(text,id));let _=tx.send(ok);});
        self.result=Some(ResultJob{rx,verify:true,..job});
    }
    pub fn stats(&self,since:i64)->Result<Value,String>{
        let mut st=self.db.prepare("SELECT control,sum(CASE WHEN coalesce(end_ms,1000*stamp)-coalesce(duration,0)>=1000*?1 THEN 1 ELSE 0 END),coalesce(sum(CASE WHEN interrupted=0 THEN min(duration,coalesce(end_ms,1000*stamp)-1000*?1) ELSE 0 END),0),coalesce(max(CASE WHEN interrupted=0 THEN duration ELSE 0 END),0),sum(interrupted) FROM usage WHERE stamp>=?1 GROUP BY control").map_err(|e|e.to_string())?;
        let rows=st.query_map([since],|r|Ok(json!({"control":r.get::<_,String>(0)?,"count":r.get::<_,i64>(1)?,"ms":r.get::<_,i64>(2)?,"longest":r.get::<_,i64>(3)?,"interrupted":r.get::<_,i64>(4)?}))).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>,_>>().map(|v|json!(v)).map_err(|e|e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    struct Fixture(PathBuf, Option<Runtime>);
    impl Fixture {
        fn new() -> Self {
            let dir=std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
            let app=Runtime::open(&dir).unwrap();
            Self(dir,Some(app))
        }
        fn app(&mut self)->&mut Runtime{self.1.as_mut().unwrap()}
    }
    impl Drop for Fixture {fn drop(&mut self){drop(self.1.take());let _=std::fs::remove_dir_all(&self.0);}}
    #[test]
    fn save_is_atomic_and_revision_checked(){
        let mut tmp=Fixture::new();let d=tmp.app();let mut draft=d.config.clone();let old=draft.clone();
        draft.tool="自定义".into();d.save(draft).unwrap();assert!(d.save(old).is_err());
        let mut broken=d.config.clone();broken.active="missing".into();assert!(d.save(broken).is_err());
        let again=Runtime::open(&tmp.0).unwrap();assert_eq!(again.config.tool,"自定义");assert_eq!(again.config.revision,1);
    }
    #[test]
    fn setup_only_opens_automatically_for_new_installations(){
        let fresh=Config::default();
        assert!(!fresh.setup_complete);
        let mut old=serde_json::to_value(&fresh).unwrap();
        old.as_object_mut().unwrap().remove("setupComplete");
        let upgraded:Config=serde_json::from_value(old).unwrap();
        assert!(upgraded.setup_complete);
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires AU05_TEST_LAUNCH_TARGET and an interactive desktop"]
    fn dial_and_sequence_open_same_application() {
        let target=std::env::var("AU05_TEST_LAUNCH_TARGET").unwrap();
        let mut fixture=Fixture::new();
        let desk=fixture.app();
        let launch=Action::Launch{app:target.clone(),name:None};
        desk.config.profiles[0].bindings.insert("DIAL.single".into(),launch.clone());
        desk.bindings.insert("test dial".into(),"DIAL".into());
        desk.input("test dial","DIAL","down",0).unwrap();
        desk.input("test dial","DIAL","up",0).unwrap();
        std::thread::sleep(Duration::from_millis(310));
        desk.tick();
        let until=Instant::now()+Duration::from_secs(10);
        while desk.launcher.waiting_for(&target) {
            assert!(Instant::now()<until);
            std::thread::sleep(Duration::from_millis(50));desk.tick();
        }
        assert!(platform::launch_is_foreground(&target,&platform::foreground()));
        desk.config.sequences.insert("test".into(),crate::model::Sequence{
            name:"test".into(),steps:vec![launch,Action::Wait{ms:10}]
        });
        desk.try_sequence("test".into()).unwrap();
        while desk.run.is_some() {
            assert!(Instant::now()<until);
            std::thread::sleep(Duration::from_millis(50));desk.tick();
        }
        assert_eq!(desk.message,"组合执行完毕");
    }
    #[test]
    fn diagnostic_events_never_count_or_execute(){
        let mut tmp=Fixture::new();let d=tmp.app();
        d.bindings.insert("one".into(),"K1".into());d.set_diagnostic(true);d.config.voice_enabled=true;
        d.input("one","K1","down",0).unwrap();d.input("one","K1","up",0).unwrap();
        assert_eq!(d.event_count,2);assert!(d.voice.is_none());assert_eq!(d.stats(0).unwrap(),json!([]));
        assert_eq!(d.snapshot()["inputTest"]["pairs"]["K1"],1);
        d.set_diagnostic(true);
        assert_eq!(d.snapshot()["inputTest"]["pairs"]["K1"],1);
        d.set_diagnostic(false);d.set_diagnostic(true);
        assert!(d.snapshot()["inputTest"]["pairs"]["K1"].is_null());
        d.cleanup();assert!(d.bindings.is_empty());assert!(!d.online);
    }
    #[test]
    fn grant_revocation_is_client_specific(){
        let mut tmp=Fixture::new();let d=tmp.app();crate::agent::setup(&d.db).unwrap();
        crate::agent::grant(&d.db,"桌面 A").unwrap();crate::agent::grant(&d.db,"桌面 B").unwrap();
        let grants=crate::agent::list(&d.db).unwrap();assert_eq!(grants.len(),2);
        d.db.execute("DELETE FROM grants WHERE id=?",[grants[0]["id"].as_str().unwrap()]).unwrap();
        assert_eq!(crate::agent::list(&d.db).unwrap().len(),1);
    }
}
