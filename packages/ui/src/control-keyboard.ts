export function nextControlIndex(current: number, maximum: number, key: string): number | null {
  let next: number;
  switch (key) {
    case 'ArrowUp':
    case 'ArrowRight':
    case '+': next = current + 1; break;
    case 'ArrowDown':
    case 'ArrowLeft':
    case '-': next = current - 1; break;
    case 'PageUp': next = current + 10; break;
    case 'PageDown': next = current - 10; break;
    case 'Home': next = 0; break;
    case 'End': next = maximum; break;
    default: return null;
  }
  return Math.max(0, Math.min(maximum, next));
}
