/* SPDX-License-Identifier: Apache-2.0 */

import confetti from "canvas-confetti";

const COLORS = ["#0FBF3E", "#3ddc6e", "#0a9630"];

export function burstConfettiAt(element: HTMLElement): void {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return;
  }

  const { left, top, width, height } = element.getBoundingClientRect();
  const x = (left + width / 2) / window.innerWidth;
  const y = (top + height / 2) / window.innerHeight;

  confetti({
    particleCount: 15,
    spread: 55,
    startVelocity: 20,
    ticks: 60,
    gravity: 1.2,
    scalar: 0.6,
    origin: { x, y },
    colors: COLORS,
    disableForReducedMotion: true,
  });
}
