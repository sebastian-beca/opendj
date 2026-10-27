"""A synthetic test mix for stem-bench: kick and hi-hats, bass, chords and a
sung-like vowel line. Usage: make_test_mix.py OUT.wav [SECONDS]"""
import numpy as np, soundfile as sf, sys
sr=44100; n=int(sr*float(sys.argv[2] if len(sys.argv)>2 else 30)); t=np.arange(n)/sr; rng=np.random.default_rng(1)
beat=0.5; x=np.zeros((n,2),np.float32)
for b in np.arange(0,n/sr,beat):
    i=int(b*sr); L=int(0.25*sr); tt=np.arange(L)/sr
    k=np.sin(2*np.pi*(50+120*np.exp(-tt*30))*tt)*np.exp(-tt*12); x[i:i+L]+=0.8*k[:len(x[i:i+L]),None]
    j=int((b+beat/2)*sr); L2=int(0.05*sr); h=rng.normal(size=L2)*np.exp(-np.arange(L2)/sr*80); h=np.diff(h,prepend=0)
    x[j:j+L2]+=0.15*h[:len(x[j:j+L2]),None]
f0=np.array([55,55,73.4,65.4])[(t//2).astype(int)%4]
x+=0.3*np.sign(np.sin(2*np.pi*np.cumsum(f0)/sr))[:,None]*0.5
for m in [1,1.26,1.5]: x[:,0]+=0.07*np.sin(2*np.pi*220*m*t+1); x[:,1]+=0.07*np.sin(2*np.pi*220*m*t)
v=440*(1+0.01*np.sin(2*np.pi*5*t))*np.array([1,1.12,1.26,1.12])[(t//1).astype(int)%4]; ph=2*np.pi*np.cumsum(v)/sr
voc=sum(np.sin(k*ph)/k*np.exp(-((k*v-700)/300)**2) for k in range(1,20)); x+=0.4*voc[:,None]
x/=np.abs(x).max()*1.1; sf.write(sys.argv[1],x.astype(np.float32),sr,subtype="FLOAT")
