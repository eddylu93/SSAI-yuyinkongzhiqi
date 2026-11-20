import * as THREE from 'three';
import {RoundedBoxGeometry} from 'three/addons/geometries/RoundedBoxGeometry.js';
import {mergeGeometries} from 'three/addons/utils/BufferGeometryUtils.js';

// Millimetres, with the front normal facing +Z. Overall envelope 30 × 112 × 28.
export const lampNames=['语音键','确认键','取消键','旋钮'];
export const keyRows=[-2.8,-22.3,-41.8];
const dialY=22;
export const wirePoints=[27.2,25.0,22.8,20.6,18.4].map(y=>[-12.7,y,16.9])
  .concat(keyRows.map(y=>[9.1,y,13.7]));
export const hitPoints=[[-9.5,25,18.5],[0,22,18.5],[9.5,25,18.5],[-7.5,16,18.5],[7.5,16,18.5],
  ...keyRows.map(y=>[0,y,15.3])];

export function createDevice(){
  const body=new THREE.Group();
  const finish=(color,metalness=0.12,roughness=0.48)=>new THREE.MeshStandardMaterial({color,metalness,roughness});
  const satin=finish(0xe2e3e5,.27,.53);
  const dialRim=finish(0xebebee,.38,.32);
  const edge=finish(0xd4d6d9,.28,.43),keys=finish(0xdbdbdb,.02,.73);
  const graphite=finish(0x494b4d),recess=finish(0x444748),engraving=finish(0x343538);
  // Fine, isotropic grain on the bead-blasted housing and dial.
  const grain=document.createElement('canvas');grain.width=grain.height=128;
  const gc=grain.getContext('2d'),pixels=gc.createImageData(128,128);
  let seed=17;
  for(let i=0;i<128*128;i++){
    seed=(Math.imul(seed,1664525)+1013904223)>>>0;
    const v=232+((seed>>>24)/255)*20;
    pixels.data.set([v,v,v,255],i*4);
  }
  gc.putImageData(pixels,0,0);
  const grainMap=new THREE.CanvasTexture(grain);grainMap.colorSpace=THREE.SRGBColorSpace;
  grainMap.wrapS=grainMap.wrapT=THREE.RepeatWrapping;
  satin.map=grainMap;
  function mesh(geometry,material,x,y,z){
    // Project the grain in millimetres so it has the same scale on the long sides and dial.
    if(material===satin){
      const {position,normal,uv}=geometry.attributes;
      for(let i=0;i<position.count;i++){
        const nx=Math.abs(normal.getX(i)),ny=Math.abs(normal.getY(i)),nz=Math.abs(normal.getZ(i));
        const u=nx>nz&&nx>ny?position.getZ(i):position.getX(i);
        const v=ny>nz&&ny>=nx?position.getZ(i):position.getY(i);
        uv.setXY(i,u/32,v/32);
      }
    }
    const part=new THREE.Mesh(geometry,material);
    part.position.set(x,y,z);body.add(part);return part;
  }
  function box(w,h,d,r,material,x,y,z){
    const geometry=Math.min(w,h,d)<.5 ? new THREE.BoxGeometry(w,h,d)
      : new RoundedBoxGeometry(w,h,d,2,r);
    return mesh(geometry,material,x,y,z);
  }
  function disc(radius,depth,material,x,y,z,top=radius){
    const g=new THREE.CylinderGeometry(top,radius,depth,radius<2?12:64);
    g.rotateX(Math.PI/2);return mesh(g,material,x,y,z);
  }
  function marking(points,y,z,material=engraving,width=.21){
    for(let i=1;i<points.length;i++){
      const [ax,ay]=points[i-1],[bx,by]=points[i];
      const line=box(Math.hypot(bx-ax,by-ay)+width,width*2,.12,.055,material,(ax+bx)/2,y+(ay+by)/2,z);
      line.rotation.z=Math.atan2(by-ay,bx-ax);
    }
  }
  function arc(x,y,r,a,b,row,z,width){
    const pts=Array.from({length:19},(_,i)=>{const t=a+(b-a)*i/18;return [x+r*Math.cos(t),y+r*Math.sin(t)]});
    marking(pts,row,z,engraving,width);
  }
  // Face coordinates use local XY. Their positive Z always points out of the housing.
  const front=[0,0,0], rear=[0,Math.PI,0], top=[-Math.PI/2,0,0], bottom=[Math.PI/2,0,0];
  function roundedFace(w,h,r){
    const x=w/2,y=h/2,shape=new THREE.Shape();
    shape.moveTo(-x+r,-y);
    shape.lineTo(x-r,-y);shape.quadraticCurveTo(x,-y,x,-y+r);
    shape.lineTo(x,y-r);shape.quadraticCurveTo(x,y,x-r,y);
    shape.lineTo(-x+r,y);shape.quadraticCurveTo(-x,y,-x,y-r);
    shape.lineTo(-x,-y+r);shape.quadraticCurveTo(-x,-y,-x+r,-y);
    return shape;
  }
  function face(shape,material,at,orientation,offset=0){
    const part=mesh(new THREE.ShapeGeometry(shape,5),material,...at);
    part.rotation.set(...orientation);
    part.translateZ(offset);
    return part;
  }
  function label(text,w,h,at,orientation,angle=0){
    const c=document.createElement('canvas');c.width=512;c.height=96;
    const ctx=c.getContext('2d');ctx.fillStyle='#666865';ctx.textAlign='center';ctx.textBaseline='middle';
    ctx.font='500 78px Arial, sans-serif';
    ctx.translate(256,48);ctx.scale(490/ctx.measureText(text).width,1);ctx.fillText(text,0,0);
    const map=new THREE.CanvasTexture(c);map.colorSpace=THREE.SRGBColorSpace;
    const m=new THREE.MeshBasicMaterial({map,transparent:true,depthWrite:false});
    const tag=mesh(new THREE.PlaneGeometry(w,h),m,...at);
    tag.rotation.set(...orientation);tag.rotateZ(angle);tag.userData.decal=true;
  }
  const dark=finish(0x242628,.06,.75), meshMetal=finish(0xd6d7da,.28,.46);
  function grille(w,h,r,spacing,bore,at,orientation){
    face(roundedFace(w+.18,h+.18,r+.09),dark,at,orientation);
    const panel=roundedFace(w,h,r);
    const dy=spacing*.866;
    for(let row=-Math.floor(h/(2*dy));row<=Math.floor(h/(2*dy));row++){
      for(let col=-Math.ceil(w/(2*spacing));col<=Math.ceil(w/(2*spacing));col++){
        const x=(col+(Math.abs(row)%2)*.5)*spacing,y=row*dy;
        const dx=Math.max(Math.abs(x)-(w/2-r),0),ey=Math.max(Math.abs(y)-(h/2-r),0);
        if(Math.hypot(dx,ey)+bore+.07>r)continue;
        const hole=new THREE.Path();hole.absarc(x,y,bore,0,Math.PI*2,true);panel.holes.push(hole);
      }
    }
    face(panel,meshMetal,at,orientation,.075);
  }

  box(30,112,17.8,1.0,satin,0,0,0);
  for(let n=0;n<6;n++){
    grille(2.1,n===0||n===5?6.1:7.9,1.02,.81,.26,[(n-2.5)*4,43,8.94],front);
  }
  grille(24.8,11.8,2.1,1.12,.36,[0,56.05,0],top);
  for(const sign of [-1,1]){
    const side=[0,sign*Math.PI/2,0];
    for(const z of [-1.95,1.95])grille(2.15,6.15,1.05,.83,.27,[sign*15.05,45,z],side);
  }
  const right=[0,Math.PI/2,0];
  face(roundedFace(5.45,5.45,2.725),recess,[15.05,37,0],right);
  const power=disc(2.54,.29,satin,15.14,37,0,2.43);power.rotation.y=Math.PI/2;
  label('POWER',6.7,1.8,[15.07,29,0],right,Math.PI/2);
  face(roundedFace(4.3,7.0,2.0),edge,[15.05,-42,0],right);
  face(roundedFace(3.65,6.25,1.72),dark,[15.12,-42,0],right);
  face(roundedFace(2.95,.82,.37),meshMetal,[15.18,-42,0],right);

  face(roundedFace(9.35,3.55,1.68),edge,[0,-56.04,0],bottom);
  face(roundedFace(8.65,2.95,1.40),dark,[0,-56.11,0],bottom);
  face(roundedFace(6.0,.63,.3),graphite,[0,-56.17,0],bottom);
  face(roundedFace(1,1,.5),dark,[0,-56.08,3.2],bottom);

  face(roundedFace(29.1,111.1,2),edge,[0,0,-8.95],rear);
  face(roundedFace(28.55,110.5,1.8),finish(0xd9d9db,.14,.60),[0,0,-9.01],rear);
  const rearPrint=(text,w,h,x,y)=>label(text,w,h,[x,y,-9.08],rear,-Math.PI/2);
  rearPrint('Ulanzi',12.2,3.0,-8.4,41);
  rearPrint('Model: AU05',13,1.35,-5.45,40.6);
  rearPrint('Shenzhen Ulanzi Technology Co., Ltd.',30,1.15,-3.35,32);
  rearPrint('FCC   CE   RoHS',31,3.0,5.3,-29);
  rearPrint('Shenzhen Ulanzi Technology Co., Ltd.',29,1.12,8.2,-29);
  const lamps=Array.from({length:4},()=>finish(0xaaa59a,0,.54));
  // Feather the light spilling onto the chassis. The lens geometry remains
  // visible from the side and occludes correctly behind keys and the knob.
  const halos=lamps.map((_,i)=>{
    const c=document.createElement('canvas');c.width=c.height=192;const ctx=c.getContext('2d');
    ctx.lineWidth=7;ctx.strokeStyle='#ffdf9c';ctx.shadowColor='#ffc675';ctx.shadowBlur=12;
    ctx.beginPath();
    if(i===3)ctx.arc(96,96,78,0,Math.PI*2);
    else ctx.roundRect(16,16,160,160,7);
    ctx.stroke();
    const map=new THREE.CanvasTexture(c);map.colorSpace=THREE.SRGBColorSpace;
    const mat=new THREE.MeshBasicMaterial({map,transparent:true,depthWrite:false,opacity:0});
    const glow=mesh(new THREE.PlaneGeometry(i===3?31.4:23,i===3?31.4:22.3),mat,0,i===3?dialY:keyRows[i],9.18);
    glow.userData.decal=true;return mat;
  });
  disc(13.65,1.2,lamps[3],0,dialY,9.68);
  disc(12.63,1.65,edge,0,dialY,11.1);
  disc(13.38,6.6,dialRim,0,dialY,14.92);
  disc(13.38,.96,satin,0,dialY,18.56,13.04);
  for(const a of [70,160,250,340]){
    const t=a*Math.PI/180;
    marking([[10.65*Math.cos(t),10.65*Math.sin(t)],[12.20*Math.cos(t),12.20*Math.sin(t)]],dialY,19.08,engraving,.095);
  }
  face(roundedFace(20.35,59.05,.95),dark,[0,keyRows[1],8.96],front);
  keyRows.forEach((y,i)=>{
    box(20,19.22,1.24,.55,lamps[i],0,y,9.58);
    const key=box(19.65,19.06,4.02,.56,keys,0,y,12.03);
    // The moulded skirt narrows slightly towards its shallow circular crown.
    const vertices=key.geometry.attributes.position;
    for(let n=0;n<vertices.count;n++){
      const t=(vertices.getZ(n)+2.01)/4.02,k=1-.037*t;
      vertices.setX(n,vertices.getX(n)*k);vertices.setY(n,vertices.getY(n)*k);
    }
    key.geometry.computeVertexNormals();
    disc(7.12,1.14,keys,0,y,14.53,6.86);
    disc(6.87,.34,keys,0,y,15.18,6.56);
    const z=15.42;
    if(i===0){
      arc(0,1.11,.74,0,Math.PI,y,z,.15);arc(0,.08,.74,Math.PI,2*Math.PI,y,z,.15);
      for(const x of [-.74,.74])marking([[x,.08],[x,1.11]],y,z,engraving,.15);
      arc(0,.0,1.44,Math.PI,2*Math.PI,y,z,.16);
      marking([[0,-1.42],[0,-2.0]],y,z,engraving,.17);
    } else {
      arc(0,0,1.91,.96,3.48,y,z,.17);
      arc(0,0,1.91,4.04,6.42,y,z,.17);
      if(i===1)marking([[-.96,-.05],[-.24,-.78],[1.8,1.26]],y,z);
      else{marking([[-.68,.68],[.68,-.68]],y,z);marking([[-.68,-.68],[.68,.68]],y,z)}
    }
  });
  // Draw the stationary details in material batches, including perforations
  // and engraved strokes, so an animated lamp costs only a few draw calls.
  const batches=new Map();
  for(const part of [...body.children]){
    if(part.userData.decal)continue;
    part.updateMatrix();
    const g=part.geometry.index?part.geometry.toNonIndexed():part.geometry;
    if(g!==part.geometry)part.geometry.dispose();
    g.applyMatrix4(part.matrix);body.remove(part);
    if(!batches.has(part.material))batches.set(part.material,[]);
    batches.get(part.material).push(g);
  }
  for(const [material,parts] of batches){
    const geometry=mergeGeometries(parts);
    for(const g of parts)g.dispose();
    body.add(new THREE.Mesh(geometry,material));
  }
  return {body,lamps,halos};
}

export function disposeModel(group){
  const geometries=new Set(),materials=new Set();
  group.traverse(part=>{if(part.isMesh){geometries.add(part.geometry);materials.add(part.material)}});
  for(const g of geometries)g.dispose();
  for(const m of materials){m.map?.dispose();m.dispose()}
}
