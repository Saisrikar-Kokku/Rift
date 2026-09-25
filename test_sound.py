import math
import struct
import ctypes
import time

def make_wav(samples, sample_rate=44100):
    num_samples = len(samples)
    byte_rate = sample_rate * 2
    block_align = 2
    subchunk2_size = num_samples * 2
    chunk_size = 36 + subchunk2_size

    header = bytearray()
    header.extend(b'RIFF')
    header.extend(struct.pack('<I', chunk_size))
    header.extend(b'WAVE')
    header.extend(b'fmt ')
    header.extend(struct.pack('<I', 16))
    header.extend(struct.pack('<H', 1))  # PCM
    header.extend(struct.pack('<H', 1))  # 1 channel (mono)
    header.extend(struct.pack('<I', sample_rate))
    header.extend(struct.pack('<I', byte_rate))
    header.extend(struct.pack('<H', block_align))
    header.extend(struct.pack('<H', 16)) # 16 bits per sample
    header.extend(b'data')
    header.extend(struct.pack('<I', subchunk2_size))

    data = bytearray()
    for s in samples:
        clamped = max(-1.0, min(1.0, s))
        val = int(clamped * 32767.0)
        data.extend(struct.pack('<h', val))

    return bytes(header + data)

def gen_start_tone(volume=0.6):
    sr = 44100
    duration = 0.075 # 75ms
    num_samples = int(sr * duration)
    samples = []
    f_start = 520.0
    f_end = 760.0
    phase = 0.0

    for i in range(num_samples):
        t = i / float(sr)
        progress = t / duration
        freq = f_start + (f_end - f_start) * progress
        phase += 2.0 * math.pi * freq / sr

        # Smooth envelope: quick fade-in, gentle fade-out
        if progress < 0.15:
            env = progress / 0.15
        elif progress > 0.6:
            env = (1.0 - progress) / 0.4
        else:
            env = 1.0

        sample = math.sin(phase) * env * volume * 0.45
        samples.append(sample)

    return make_wav(samples, sr)

def gen_success_tone(volume=0.6):
    sr = 44100
    duration = 0.11 # 110ms
    num_samples = int(sr * duration)
    samples = []
    
    for i in range(num_samples):
        t = i / float(sr)
        # Two harmonic bell frequencies: 659.25Hz (E5) and 987.77Hz (B5)
        # Note 1 starts at 0ms, Note 2 starts at 35ms
        s = 0.0
        # Note 1
        if t < 0.07:
            env1 = math.exp(-t * 35.0)
            s += math.sin(2.0 * math.pi * 659.25 * t) * env1 * 0.6
        # Note 2
        if t >= 0.035:
            t2 = t - 0.035
            env2 = math.exp(-t2 * 30.0)
            s += math.sin(2.0 * math.pi * 987.77 * t2) * env2 * 0.7

        sample = s * volume * 0.4
        samples.append(sample)

    return make_wav(samples, sr)

def gen_cancel_tone(volume=0.6):
    sr = 44100
    duration = 0.05 # 50ms
    num_samples = int(sr * duration)
    samples = []
    for i in range(num_samples):
        t = i / float(sr)
        progress = t / duration
        freq = 320.0 - 140.0 * progress
        env = (1.0 - progress)
        s = math.sin(2.0 * math.pi * freq * t) * env * volume * 0.35
        samples.append(s)
    return make_wav(samples, sr)

winmm = ctypes.windll.winmm
SND_ASYNC = 0x0001
SND_NODEFAULT = 0x0002
SND_MEMORY = 0x0004
FLAGS = SND_ASYNC | SND_NODEFAULT | SND_MEMORY

w_start = gen_start_tone(0.6)
w_success = gen_success_tone(0.6)
w_cancel = gen_cancel_tone(0.6)

print("Playing start tone...")
winmm.PlaySoundW(w_start, 0, FLAGS)
time.sleep(0.5)

print("Playing success tone...")
winmm.PlaySoundW(w_success, 0, FLAGS)
time.sleep(0.5)

print("Playing cancel tone...")
winmm.PlaySoundW(w_cancel, 0, FLAGS)
time.sleep(0.5)
print("Done!")
