// Lógica pura de propiedad de notas/pads y de sesiones MIDI (A04).
// Se mantiene fuera del hook de React para poder probarla sin DOM (ver tests/midiOwnership.test.ts).

export type HeldMap = Map<number, number>;

/** Puertos sintéticos con los que el backend reemite las pulsaciones de la interfaz virtual. */
const VIRTUAL_PORT_NAMES = new Set(["Virtual Input", "Virtual Pad"]);

export function isVirtualPortName(portName: string | undefined | null): boolean {
  return !!portName && VIRTUAL_PORT_NAMES.has(portName);
}

export type SessionVerdict = "virtual" | "stale" | "current" | "newer";

/**
 * Clasifica un evento según la época de sesión MIDI (monotónica, asignada por Rust).
 * - `virtual`: originado en la interfaz virtual; no pertenece a ninguna sesión física.
 * - `stale`: época menor a la vigente (evento tardío de una conexión anterior): se ignora.
 * - `newer`: época mayor: una conexión nueva empezó antes de recibir su `midi-status-changed`.
 */
export function classifyMidiSession(
  currentSession: number,
  eventSession: number | undefined,
  portName?: string | null,
): SessionVerdict {
  if (isVirtualPortName(portName) || !eventSession) return "virtual";
  if (eventSession < currentSession) return "stale";
  if (eventSession > currentSession) return "newer";
  return "current";
}

/** Une dos mapas de teclas/pads sostenidos; una tecla sostenida por cualquiera sigue activa. */
export function mergeHeld(physical: HeldMap, virtual: HeldMap): HeldMap {
  if (virtual.size === 0) return physical;
  if (physical.size === 0) return virtual;
  const merged = new Map(physical);
  virtual.forEach((vel, key) => merged.set(key, vel));
  return merged;
}

/** Devuelve el mismo mapa si ya está vacío (evita renders innecesarios al limpiar). */
export function clearHeld(map: HeldMap): HeldMap {
  return map.size === 0 ? map : new Map();
}
