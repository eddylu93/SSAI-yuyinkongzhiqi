import targets from './focus-targets.json';
export const groups=[
 {control:'DIAL',name:'旋钮左旋',gestures:['left'],y:.30},
 {control:'DIAL',name:'旋钮按下',gestures:['single','double','long'],y:.305},
 {control:'DIAL',name:'旋钮右旋',gestures:['right'],y:.31},
 {control:'DIAL',name:'按下左旋',gestures:['heldLeft'],y:.33},
 {control:'DIAL',name:'按下右旋',gestures:['heldRight'],y:.34},
 {control:'K1',name:'语音键',gestures:[],y:.533},
 {control:'K2',name:'确认键',gestures:['single','double','long'],y:.70},
 {control:'K3',name:'取消键',gestures:['single','double','long'],y:.867}
];
export const gestures={single:'单击',double:'双击',long:'长按',left:'左旋',right:'右旋',heldLeft:'按下左旋',heldRight:'按下右旋'};
export function describe(a,c){
 if(!a)return '未设置';
 switch(a.kind){case 'shortcut':return a.keys.join(' + ');case 'scroll':return a.steps>0?'向上滚动':'向下滚动';case 'focusChat':return '聚焦 '+(targets.find(t=>t.id===a.target)?.name??'聊天框');case 'focusCodex':return '聚焦 Codex 输入框';case 'launch':return a.name || a.app.split(/[\\/]/).pop();case 'text':return a.phrase;case 'sequence':return c.sequences[a.id]?.name??'组合';case 'wait':return a.ms+' ms';default:return '未设置'}
}
