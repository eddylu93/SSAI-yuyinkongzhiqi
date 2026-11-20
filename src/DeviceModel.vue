<script setup>
import {computed,nextTick,onBeforeUnmount,onMounted,ref,watch} from 'vue';
import * as THREE from 'three';
import {SVGRenderer} from 'three/addons/renderers/SVGRenderer.js';
import {createDevice,disposeModel,wirePoints,hitPoints,lampNames} from './deviceModel.js';
import {groups} from './mappings.js';

const props=defineProps({lights:Object,connected:Boolean,picked:Number});
const emit=defineEmits(['projection','pick']);
const surface=ref(null),viewport=ref(null),spots=ref([]);
const effectNames=['灭','常亮','呼吸'];
const effects=computed(()=>lampNames.map((_,i)=>{
  if(!props.connected||!props.lights)return 0;
  const s=props.lights;
  return s.mode===2?s.lamps[i]:Number(s.mode===1&&s.brightness>0);
}));
const lightLabel=computed(()=>!props.connected?'机身未连接':!props.lights?'等待设备灯光回读':
    lampNames.map((name,i)=>name+' '+effectNames[effects.value[i]]).join('，'));
let scene,camera,device,renderer,software=false,resizeObserver,visibilityObserver,frame=0,visible=true,disposed=false;
let yaw=0,pitch=0,targetYaw=0,targetPitch=0;
let drag=null,blockClickUntil=0,lastRender=-1000,dirty=true,projectionDirty=true;
const motion=window.matchMedia('(prefers-reduced-motion: reduce)');
const clamp=THREE.MathUtils.clamp;
function redraw(){dirty=true;if(!disposed&&!frame&&visible&&!document.hidden)frame=requestAnimationFrame(draw)}
function project(points){
  return points.map(p=>{
    const v=new THREE.Vector3(...p).applyMatrix4(device.body.matrixWorld).project(camera);
    return [(v.x+1)/2,(1-v.y)/2];
  });
}
function draw(time){
  frame=0;
  if(disposed||!renderer||!visible||document.hidden)return;
  const moving=Math.abs(targetYaw-yaw)+Math.abs(targetPitch-pitch)>.08;
  const breathing=effects.value.includes(2)&&!motion.matches;
  const step=software?80:32;
  if(dirty||time-lastRender>=step){
    if(moving||projectionDirty){
      const ease=motion.matches?1:.37;
      yaw+=(targetYaw-yaw)*ease;pitch+=(targetPitch-pitch)*ease;
      if(Math.abs(targetYaw-yaw)<.08)yaw=targetYaw;
      if(Math.abs(targetPitch-pitch)<.08)pitch=targetPitch;
      device.body.rotation.set(pitch*Math.PI/180,yaw*Math.PI/180,0,'YXZ');
      device.body.updateMatrixWorld(true);
      const front=Math.cos(yaw*Math.PI/180)*Math.cos(pitch*Math.PI/180)>.59;
      spots.value=front?project(hitPoints):[];
      emit('projection',front?project(wirePoints):[]);
      projectionDirty=false;
    }
    effects.value.forEach((effect,i)=>{
      const lamp=device.lamps[i],s=props.lights;
      const level=s?.mode===1?s.brightness:(s?.workBrightness?.[i]??2);
      const pulse=effect===2&&!motion.matches? .18+.82*(1-Math.cos(time*2*Math.PI/2300))/2:1;
      const intensity=effect?(level===0?0:level===1?.44:1)*pulse:0;
      lamp.color.setRGB(.31+.65*intensity,.30+.44*intensity,.27+.16*intensity);
      lamp.emissive.setRGB(intensity*.85,intensity*.50,intensity*.16);
      device.halos[i].opacity=intensity*.85;
    });
    renderer.render(scene,camera);
    if(software)renderer.domElement.style.backgroundColor='transparent';
    lastRender=time;dirty=false;
  }
  if(moving||breathing)frame=requestAnimationFrame(draw);
}
function resize(){
  if(!renderer||!viewport.value)return;
  const w=viewport.value.clientWidth,h=viewport.value.clientHeight;
  if(!w||!h)return;
  const height=119;camera.left=-height*w/h/2;camera.right=-camera.left;
  camera.top=height/2;camera.bottom=-height/2;camera.updateProjectionMatrix();
  renderer.setSize(w,h);projectionDirty=true;redraw();
}
function pointerDown(e){
  if(e.button!==0||drag)return;
  const hotspot=e.target.closest('.hot');
  surface.value.setPointerCapture(e.pointerId);
  drag={id:e.pointerId,tap:hotspot?Number(hotspot.dataset.index):null,
    x:e.clientX,y:e.clientY,yaw,pitch,moved:false};
}
function pointerMove(e){
  if(!drag||e.pointerId!==drag.id)return;
  const dx=e.clientX-drag.x,dy=e.clientY-drag.y;
  if(!drag.moved&&Math.hypot(dx,dy)<5)return;
  drag.moved=true;
  targetYaw=drag.yaw+dx*1.2;targetPitch=clamp(drag.pitch+dy*.6,-90,90);
  redraw();
}
function pointerUp(e){
  if(!drag||drag.id!==e.pointerId)return;
  if(drag.moved)blockClickUntil=performance.now()+280;
  if(!drag.moved&&drag.tap!==null&&e.type==='pointerup')pick(drag.tap);
  drag=null;
  if(surface.value.hasPointerCapture(e.pointerId))surface.value.releasePointerCapture(e.pointerId);
}
function rotateKey(e){
  if(e.key==='Home'){e.preventDefault();targetYaw=0;targetPitch=0;redraw();return}
  if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;
  e.preventDefault();
  targetYaw+=e.key==='ArrowLeft'?-15:e.key==='ArrowRight'?15:0;
  targetPitch=clamp(targetPitch+(e.key==='ArrowUp'?-10:e.key==='ArrowDown'?10:0),-90,90);
  redraw();
}
function pick(i){if(performance.now()>blockClickUntil)emit('pick',i)}
function visibility(){if(document.hidden||!visible){cancelAnimationFrame(frame);frame=0}else redraw()}
function useSoftware(){
  software=true;renderer=new SVGRenderer();renderer.setPrecision(2);
  // SVGRenderer uses unnormalised Lambert lighting and ignores ambient intensity.
  for(const light of scene.children){
    if(light.isAmbientLight)light.color.multiplyScalar(light.intensity/Math.PI);
    if(light.isDirectionalLight)light.intensity/=Math.PI;
  }
  const materials=new Set();
  device.body.traverse(part=>{
    if(part.userData.decal)part.visible=false;
    else if(part.material?.isMeshStandardMaterial)materials.add(part.material);
  });
  // The vector renderer uses only diffuse colour. Match the metal's diffuse share.
  for(const material of materials)material.color.multiplyScalar(1-material.metalness);
}
function lost(e){
  e.preventDefault();renderer.domElement.removeEventListener('webglcontextlost',lost);
  renderer.dispose();renderer.domElement.remove();useSoftware();attachRenderer();
}
function attachRenderer(){
  renderer.domElement.setAttribute('aria-hidden','true');
  viewport.value.append(renderer.domElement);resize();
}
onMounted(()=>{
  scene=new THREE.Scene();
  camera=new THREE.OrthographicCamera(-20,20,60,-60,.1,600);camera.position.z=210;camera.updateMatrixWorld();
  scene.add(new THREE.AmbientLight(0xffffff,1.15));
  const key=new THREE.DirectionalLight(0xffffff,2.35);key.position.set(-80,105,160);scene.add(key);
  const fill=new THREE.DirectionalLight(0xf9faff,1.1);fill.position.set(80,-30,65);scene.add(fill);
  const rim=new THREE.DirectionalLight(0xffffff,1.55);rim.position.set(20,80,-100);scene.add(rim);
  device=createDevice();scene.add(device.body);
  try{
    renderer=new THREE.WebGLRenderer({antialias:true,alpha:true,powerPreference:'low-power'});
    renderer.setPixelRatio(Math.min(window.devicePixelRatio||1,2));renderer.setClearColor(0x000000,0);
    renderer.domElement.addEventListener('webglcontextlost',lost);
  }catch{useSoftware()}
  attachRenderer();
  resizeObserver=new ResizeObserver(resize);resizeObserver.observe(viewport.value);
  visibilityObserver=new IntersectionObserver(([entry])=>{visible=entry.isIntersecting;visibility()});visibilityObserver.observe(surface.value);
  motion.addEventListener('change',redraw);document.addEventListener('visibilitychange',visibility);nextTick(redraw);
});
watch(()=>[props.lights,props.connected],redraw);
onBeforeUnmount(()=>{
  disposed=true;cancelAnimationFrame(frame);resizeObserver?.disconnect();visibilityObserver?.disconnect();
  motion.removeEventListener('change',redraw);document.removeEventListener('visibilitychange',visibility);
  renderer?.domElement.removeEventListener('webglcontextlost',lost);renderer?.dispose?.();renderer?.forceContextLoss?.();
  if(device)disposeModel(device.body);
});
</script>
<template><div class="product-view">
  <div ref="surface" class="model-surface" tabindex="0" role="group" aria-label="AU05 三维模型，拖动旋转，方向键调整视角，Home 回到正面"
    @pointerdown="pointerDown" @pointermove="pointerMove" @pointerup="pointerUp" @lostpointercapture="pointerUp" @pointercancel="pointerUp" @keydown="rotateKey">
    <div ref="viewport" class="model-viewport" :title="lightLabel"></div>
    <button v-for="([x,y],i) in spots" :key="i" :data-index="i" class="hot" :class="{keycap:i>=5}" :aria-label="groups[i].name" :aria-pressed="props.picked===i" :title="groups[i].name"
      :style="{left:x*100+'%',top:y*100+'%'}" @click="pick(i)"></button>
  </div>
</div></template>
