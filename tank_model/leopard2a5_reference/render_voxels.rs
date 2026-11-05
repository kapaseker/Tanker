// Dependency-free voxel ray tracer. Input is reconstructed from the exported VOX.
use std::{fs, io::Write, ops::{Add,Sub,Mul,Div,Neg}, sync::Arc, thread};

#[derive(Clone, Copy, Default, Debug)]
struct V(f32,f32,f32);
impl Add for V {type Output=V;fn add(self,b:V)->V{V(self.0+b.0,self.1+b.1,self.2+b.2)}}
impl Sub for V {type Output=V;fn sub(self,b:V)->V{V(self.0-b.0,self.1-b.1,self.2-b.2)}}
impl Mul<f32> for V {type Output=V;fn mul(self,b:f32)->V{V(self.0*b,self.1*b,self.2*b)}}
impl Div<f32> for V {type Output=V;fn div(self,b:f32)->V{self*(1./b)}}
impl Neg for V {type Output=V;fn neg(self)->V{self * -1.}}
impl V {
    fn dot(self,b:V)->f32{self.0*b.0+self.1*b.1+self.2*b.2}
    fn cross(self,b:V)->V{V(self.1*b.2-self.2*b.1,self.2*b.0-self.0*b.2,self.0*b.1-self.1*b.0)}
    fn unit(self)->V{self/self.dot(self).sqrt()}
    fn mul(self,b:V)->V{V(self.0*b.0,self.1*b.1,self.2*b.2)}
    fn array(self)->[f32;3]{[self.0,self.1,self.2]}
}
struct Volume {n:[usize;3],low:V,data:Vec<u8>,palette:[V;256]}
#[derive(Clone,Copy)]
struct Hit {t:f32,p:V,n:V,color:u8}
impl Volume {
    fn read(path:&str)->Self {
        let bytes=fs::read(path).expect("read volume");
        let ints:Vec<i32>=(0..6).map(|i|i32::from_le_bytes(bytes[4*i..4*i+4].try_into().unwrap())).collect();
        let n=[ints[0] as usize,ints[1] as usize,ints[2] as usize];
        assert_eq!(bytes.len(),24+1024+n.iter().product::<usize>());
        let mut palette=[V::default();256];
        for i in 0..256 {let p=24+i*4;palette[i]=V((bytes[p] as f32/255.).powf(2.2),(bytes[p+1] as f32/255.).powf(2.2),(bytes[p+2] as f32/255.).powf(2.2));}
        Self{n,low:V(ints[3] as f32,ints[4] as f32,ints[5] as f32),data:bytes[1048..].to_vec(),palette}
    }
    fn trace(&self,origin:V,direction:V,limit:f32)->Option<Hit> {
        let o=(origin-self.low).array();let d=direction.array();
        let mut near=0.0f32;let mut far=limit;let mut entry=V(0.,0.,0.);
        for a in 0..3 {
            if d[a].abs()<1e-8 {if o[a]<0.||o[a]>=self.n[a] as f32{return None;}continue;}
            let t1=-o[a]/d[a];let t2=(self.n[a] as f32-o[a])/d[a];
            let (tlo,thi)=if t1<t2{(t1,t2)}else{(t2,t1)};
            if tlo>near {near=tlo;entry=match a {0=>V(-d[a].signum(),0.,0.),1=>V(0.,-d[a].signum(),0.),_=>V(0.,0.,-d[a].signum())};}
            far=far.min(thi);if far<=near{return None;}
        }
        let t0=near+0.0004;
        let p=[o[0]+d[0]*t0,o[1]+d[1]*t0,o[2]+d[2]*t0];
        let mut cell=[p[0].floor() as i32,p[1].floor() as i32,p[2].floor() as i32];
        let mut delta=[f32::INFINITY;3];let mut next=[f32::INFINITY;3];let mut step=[0i32;3];
        for a in 0..3 {if d[a].abs()>1e-8 {step[a]=if d[a]>0.{1}else{-1};delta[a]=1./d[a].abs();let boundary=if step[a]>0{cell[a]+1}else{cell[a]};next[a]=t0+(boundary as f32-p[a])/d[a];}}
        let mut t=near;let mut normal=entry;
        loop {
            if cell[0]<0||cell[1]<0||cell[2]<0||cell[0]>=self.n[0] as i32||cell[1]>=self.n[1] as i32||cell[2]>=self.n[2] as i32||t>far{return None;}
            let index=(cell[0] as usize*self.n[1]+cell[1] as usize)*self.n[2]+cell[2] as usize;
            let color=self.data[index];
            if color!=0 {return Some(Hit{t,p:origin+direction*t,n:normal,color});}
            let axis=if next[0]<next[1] {if next[0]<next[2]{0}else{2}}else if next[1]<next[2]{1}else{2};
            t=next[axis];next[axis]+=delta[axis];cell[axis]+=step[axis];
            normal=match axis{0=>V(-step[axis] as f32,0.,0.),1=>V(0.,-step[axis] as f32,0.),_=>V(0.,0.,-step[axis] as f32)};
        }
    }
}
fn random(seed:&mut u32)->f32 {*seed^=*seed<<13;*seed^=*seed>>17;*seed^=*seed<<5;*seed as f32/u32::MAX as f32}
fn shadow(vol:&Volume,p:V,n:V,sun:V,seed:&mut u32,samples:usize)->f32 {
    let mut light=0.;
    for _ in 0..samples {
        let jitter=V(random(seed)-0.5,random(seed)-0.5,random(seed)-0.5)*0.075;
        if vol.trace(p+n*0.025,(sun+jitter).unit(),900.).is_none(){light+=1.;}
    }
    light/samples as f32
}
fn ao(vol:&Volume,hit:Hit,seed:&mut u32)->f32 {
    let n=hit.n;
    let u=if n.2.abs()>0.5 {V(1.,0.,0.)}else{V(0.,0.,1.)};
    let v=n.cross(u);
    let rotation=random(seed)*std::f32::consts::TAU;
    let mut blocked=0.;
    for i in 0..8 {
        let radius=((i as f32+0.5)/8.).sqrt();
        let theta=i as f32*2.399963+rotation;
        let direction=u*(radius*theta.cos())+v*(radius*theta.sin())+n*(1.-radius*radius).sqrt();
        if let Some(h)=vol.trace(hit.p+n*0.025,direction,33.) {blocked+=1.-h.t/45.;}
    }
    1.-blocked/8.*0.78
}
fn shade(vol:&Volume,o:V,d:V,sun:V,seed:&mut u32)->V {
    let ground_t=if d.2<0.{(-1.-o.2)/d.2}else{2000.};
    if let Some(hit)=vol.trace(o,d,ground_t) {
        let mut n=hit.n;
        // Blend the stepped faces on broad armor slopes; silhouette stays voxel exact.
        let metric=hit.p;
        let p=V((metric.0+304.)*444./616.-219.,metric.1*239./300.+0.5,
            if metric.2<92.{metric.2*81./92.}
            else if metric.2<124.{81.+(metric.2-92.)*31./32.}
            else if metric.2<184.{112.+(metric.2-124.)*72./60.}
            else if metric.2<198.{184.+(metric.2-184.)*21./14.}
            else{205.+metric.2-198.});
        if p.0>145.&&p.0<218.&&p.2>73.&&p.2<114.&&p.1.abs()<77.
            && (p.2-(113.-(p.0-146.)/1.9)).abs()<2.5 {
            n=(n*0.18+V(0.39,0.,1.).unit()*0.82).unit();
        }
        if p.0>104.&&p.0<140.&&p.2>134.&&p.2<182.&&p.1.abs()>24.&&p.1.abs()<84.
            && (p.0-(138.-(p.2-130.)*0.57)).abs()<2.5 {
            n=(n*0.18+V(1.,0.,0.95).unit()*0.82).unit();
        }
        let ambient=ao(vol,hit,seed);
        let visibility=shadow(vol,hit.p,n,sun,seed,2);
        let key=n.dot(sun).max(0.)*(0.12+visibility*0.88);
        let fill=n.dot(V(0.2,0.85,0.5).unit()).max(0.);
        let front_fill=n.dot(V(0.7,-0.3,0.5).unit()).max(0.);
        let lighting=V(0.31,0.34,0.38)*ambient + V(1.3,1.09,0.75)*key*1.35
            + V(0.15,0.30,0.30)*fill*ambient + V(0.18,0.20,0.19)*front_fill*ambient;
        let mut color=vol.palette[hit.color as usize].mul(lighting);
        // Tiny edge shading preserves the square voxel facets.
        let p=(hit.p-vol.low).array();let normal=n.array();
        let mut edge=1.0f32;
        for a in 0..3 {if normal[a].abs()<0.5{let f=p[a]-p[a].floor();edge=edge.min(f.min(1.-f));}}
        color=color*if edge<0.06{0.92}else{1.};
        let family=(hit.color-1)/16;
        if [5,6,7,9].contains(&family) {
            let half=(sun-d).unit();let spec=n.dot(half).max(0.).powf(35.)*0.12*visibility;
            color=color+V(1.,0.95,0.8)*spec;
        }
        if family==10 {color=color+vol.palette[hit.color as usize]*0.22;}
        return color;
    }
    let p=o+d*ground_t;
    let vis=shadow(vol,p,V(0.,0.,1.),sun,seed,4);
    let warmth=(0.5-p.0/1200.-p.1/1800.).clamp(0.,1.);
    let base=V(0.018,0.022,0.025)*(1.-warmth)+V(0.064,0.041,0.023)*warmth;
    let grain=(random(seed)-0.5)*0.045;
    let mut contact=1.;
    if p.0.abs()<320.&&p.1.abs()<158. {
        if let Some(h)=vol.trace(p+V(0.,0.,0.02),V(0.,0.,1.),36.){contact=0.55+0.45*(h.t/36.);}
    }
    base*(0.42+vis*0.9)*contact*(1.+grain)
}
fn tone(x:f32)->u8 {
    // Gentle exposure and highlight compression, then sRGB encoding.
    let x=x*1.3;
    let y=x/(1.+x*0.45);
    (y.max(0.).powf(1./2.2).min(1.)*255.+0.5) as u8
}
fn main(){
    let args:Vec<String>=std::env::args().collect();
    assert!(args.len()>=3,"render_voxels volume.bin preview.ppm [view=front] [width=1600] [samples=2]");
    let vol=Arc::new(Volume::read(&args[1]));
    let view=args.get(3).map(|s|s.as_str()).unwrap_or("front");
    let width:usize=args.get(4).map(|s|s.parse().unwrap()).unwrap_or(1600);
    let height=width*2/3;
    let samples:usize=args.get(5).map(|s|s.parse().unwrap()).unwrap_or(2);
    let eye=match view {"rear"=>V(-0.65,0.82,0.43),"side"=>V(0.08,-1.,0.15),"top"=>V(0.01,-0.01,1.),_=>V(0.61,-0.82,0.36)}.unit();
    let right=V(-eye.1,eye.0,0.).unit();let up=eye.cross(right).unit();
    let mut bounds=[f32::INFINITY,f32::NEG_INFINITY,f32::INFINITY,f32::NEG_INFINITY];
    for x in 0..vol.n[0] {for y in 0..vol.n[1] {for z in 0..vol.n[2] {
        if vol.data[(x*vol.n[1]+y)*vol.n[2]+z]==0{continue;}
        let p=V(x as f32,y as f32,z as f32)+vol.low;
        let r=p.dot(right);let u=p.dot(up);
        bounds[0]=bounds[0].min(r);bounds[1]=bounds[1].max(r);bounds[2]=bounds[2].min(u);bounds[3]=bounds[3].max(u);
    }}}
    let center_r=(bounds[0]+bounds[1])*0.5;let center_u=(bounds[2]+bounds[3])*0.5;
    let scale=((bounds[1]-bounds[0])/width as f32).max((bounds[3]-bounds[2])/height as f32)*1.13;
    let sun=V(-0.45,-0.55,0.88).unit();
    let threads=thread::available_parallelism().map(|n|n.get()).unwrap_or(4).min(16);
    let rows=height.div_ceil(threads);
    let started=std::time::Instant::now();
    let mut handles=vec![];
    for task in 0..threads {
        let volume=vol.clone();let y0=task*rows;let y1=((task+1)*rows).min(height);
        handles.push(thread::spawn(move ||{
            let mut pixels=vec![0u8;(y1-y0)*width*3];
            for y in y0..y1 {for x in 0..width {
                let mut color=V::default();let mut seed=((y*width+x) as u32).wrapping_mul(747796405).wrapping_add(2891336453);
                for _ in 0..samples {
                    let rx=(x as f32+random(&mut seed)-width as f32/2.)*scale+center_r;
                    let uy=(height as f32/2.-y as f32-random(&mut seed))*scale+center_u;
                    let origin=right*rx+up*uy+eye*1100.;
                    color=color+shade(&volume,origin,-eye,sun,&mut seed);
                }
                color=color/samples as f32;let i=((y-y0)*width+x)*3;
                pixels[i]=tone(color.0);pixels[i+1]=tone(color.1);pixels[i+2]=tone(color.2);
            }}
            (y0,pixels)
        }));
    }
    let mut pixels=vec![0u8;width*height*3];
    for handle in handles {let (row,p)=handle.join().expect("render thread");pixels[row*width*3..row*width*3+p.len()].copy_from_slice(&p);}
    let mut f=fs::File::create(&args[2]).unwrap();
    write!(f,"P6\n{} {}\n255\n",width,height).unwrap();f.write_all(&pixels).unwrap();
    println!("Rendered {view}: {width}x{height}, {samples} samples, {:.1}s",started.elapsed().as_secs_f32());
}
