import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface AudioOrbProps {
  audioLevel?: number;
  active?: boolean;
  size?: number;
  color?: string;
  glowColor?: string;
}

const ACTIVE_FPS = 30;
const IDLE_FPS = 12;

export const AudioOrb: React.FC<AudioOrbProps> = ({
  audioLevel = 0,
  active = false,
  size = 180,
  color = '#a78bfa',
  glowColor = '#38bdf8',
}) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const audioLevelRef = useRef(audioLevel);
  const activeRef = useRef(active);
  const reducedMotionRef = useRef(false);
  const renderStaticRef = useRef<() => void>(() => undefined);

  useEffect(() => {
    audioLevelRef.current = audioLevel;
    activeRef.current = active;
  }, [audioLevel, active]);

  useEffect(() => {
    const container = mountRef.current;
    if (!container) return;

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.z = 4.2;

    const renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: true,
      powerPreference: 'low-power',
    });
    renderer.setSize(size, size);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
    container.appendChild(renderer.domElement);

    const originalGeometry = new THREE.IcosahedronGeometry(1.4, 2);
    const geometry = originalGeometry.clone();
    const positionAttr = geometry.attributes.position;
    const basePositions = new Float32Array(positionAttr.array);

    const wireframeMaterial = new THREE.MeshBasicMaterial({
      color: new THREE.Color(color),
      wireframe: true,
      transparent: true,
      opacity: 0.35,
    });
    const orbMesh = new THREE.Mesh(geometry, wireframeMaterial);
    scene.add(orbMesh);

    const pointsMaterial = new THREE.PointsMaterial({
      color: new THREE.Color(glowColor),
      size: 0.05,
      transparent: true,
      opacity: 0.85,
    });
    const points = new THREE.Points(geometry, pointsMaterial);
    scene.add(points);

    const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
    reducedMotionRef.current = motionQuery.matches;
    let documentVisible = !document.hidden;
    let intersectsViewport = true;
    let animationFrameId: number | null = null;
    let idleTimerId: number | null = null;
    let disposed = false;
    let lastRenderedAt = 0;
    const startTime = performance.now();

    const renderFrame = (timestamp: number) => {
      const elapsed = Math.max(0, timestamp - startTime) / 1000;
      const level = Math.max(0, Math.min(1, audioLevelRef.current));
      const isActive = activeRef.current;

      orbMesh.rotation.y = elapsed * (isActive ? 0.8 : 0.25);
      orbMesh.rotation.x = Math.sin(elapsed * 0.4) * 0.3;
      points.rotation.copy(orbMesh.rotation);

      const positionArray = positionAttr.array as Float32Array;
      const pulseFactor = isActive ? 0.35 + level * 0.8 : 0.08;
      const baseFrequency = isActive ? 4 : 2;

      for (let index = 0; index < positionAttr.count; index += 1) {
        const offset = index * 3;
        const x = basePositions[offset];
        const y = basePositions[offset + 1];
        const z = basePositions[offset + 2];
        const wave = Math.sin(elapsed * baseFrequency + x * 3 + y * 2) * pulseFactor;
        const scale = 1 + wave;
        positionArray[offset] = x * scale;
        positionArray[offset + 1] = y * scale;
        positionArray[offset + 2] = z * scale;
      }

      positionAttr.needsUpdate = true;
      wireframeMaterial.opacity = isActive ? 0.65 : 0.3;
      pointsMaterial.opacity = isActive ? 0.95 : 0.6;
      renderer.render(scene, camera);
      lastRenderedAt = timestamp;
    };

    renderStaticRef.current = () => renderFrame(performance.now());

    const canAnimate = () =>
      !disposed &&
      !reducedMotionRef.current &&
      documentVisible &&
      intersectsViewport;

    const cancelScheduledFrame = () => {
      if (animationFrameId !== null) {
        cancelAnimationFrame(animationFrameId);
        animationFrameId = null;
      }
      if (idleTimerId !== null) {
        window.clearTimeout(idleTimerId);
        idleTimerId = null;
      }
    };

    const scheduleNextFrame = () => {
      if (!canAnimate() || animationFrameId !== null || idleTimerId !== null) return;

      if (activeRef.current) {
        animationFrameId = requestAnimationFrame(animate);
      } else {
        idleTimerId = window.setTimeout(() => {
          idleTimerId = null;
          if (canAnimate()) animationFrameId = requestAnimationFrame(animate);
        }, 1000 / IDLE_FPS);
      }
    };

    const animate = (timestamp: number) => {
      animationFrameId = null;
      if (!canAnimate()) return;

      const fps = activeRef.current ? ACTIVE_FPS : IDLE_FPS;
      if (timestamp - lastRenderedAt >= 1000 / fps) renderFrame(timestamp);
      scheduleNextFrame();
    };

    const resumeAnimation = () => {
      if (!canAnimate()) return;
      renderFrame(performance.now());
      scheduleNextFrame();
    };

    const handleVisibilityChange = () => {
      documentVisible = !document.hidden;
      if (documentVisible) resumeAnimation();
      else cancelScheduledFrame();
    };

    const handleMotionPreferenceChange = (event: MediaQueryListEvent) => {
      reducedMotionRef.current = event.matches;
      cancelScheduledFrame();
      if (event.matches) renderFrame(performance.now());
      else resumeAnimation();
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    if (typeof motionQuery.addEventListener === 'function') {
      motionQuery.addEventListener('change', handleMotionPreferenceChange);
    } else {
      motionQuery.addListener(handleMotionPreferenceChange);
    }

    const intersectionObserver = typeof IntersectionObserver === 'undefined'
      ? null
      : new IntersectionObserver(([entry]) => {
          intersectsViewport = entry.isIntersecting;
          if (intersectsViewport) resumeAnimation();
          else cancelScheduledFrame();
        });
    intersectionObserver?.observe(container);

    renderFrame(performance.now());
    if (!reducedMotionRef.current) scheduleNextFrame();

    return () => {
      disposed = true;
      cancelScheduledFrame();
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      if (typeof motionQuery.removeEventListener === 'function') {
        motionQuery.removeEventListener('change', handleMotionPreferenceChange);
      } else {
        motionQuery.removeListener(handleMotionPreferenceChange);
      }
      intersectionObserver?.disconnect();
      renderStaticRef.current = () => undefined;
      renderer.domElement.remove();
      originalGeometry.dispose();
      geometry.dispose();
      wireframeMaterial.dispose();
      pointsMaterial.dispose();
      renderer.dispose();
    };
  }, [size, color, glowColor]);

  return (
    <div
      ref={mountRef}
      style={{
        width: `${size}px`,
        height: `${size}px`,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        position: 'relative',
        userSelect: 'none',
      }}
    />
  );
};
