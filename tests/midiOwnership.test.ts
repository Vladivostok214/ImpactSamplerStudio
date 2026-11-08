// Pruebas de la lógica pura de sesiones/propiedad MIDI (A04).
// Ejecutar con: node --experimental-strip-types --test tests/midiOwnership.test.ts
import test from "node:test";
import assert from "node:assert/strict";
import {
  classifyMidiSession,
  clearHeld,
  isVirtualPortName,
  mergeHeld,
  type HeldMap,
} from "../src/hooks/midiOwnership.ts";

/** Simula la reducción de estado del hook ante eventos y cambios de estado. */
class Sim {
  session = 0;
  physicalNotes: HeldMap = new Map();
  virtualNotes: HeldMap = new Map();
  physicalPads: HeldMap = new Map();
  virtualPads: HeldMap = new Map();

  get notes() { return mergeHeld(this.physicalNotes, this.virtualNotes); }
  get pads() { return mergeHeld(this.physicalPads, this.virtualPads); }

  clearPhysical() {
    this.physicalNotes = clearHeld(this.physicalNotes);
    this.physicalPads = clearHeld(this.physicalPads);
  }

  status(session: number) {
    if (session > this.session) {
      this.session = session;
      this.clearPhysical();
    }
  }

  noteEvent(on: boolean, note: number, session: number | undefined, port: string) {
    const verdict = classifyMidiSession(this.session, session, port);
    if (verdict === "stale") return;
    if (verdict === "newer") { this.session = session!; this.clearPhysical(); }
    const target = verdict === "virtual" ? "virtualNotes" : "physicalNotes";
    const next = new Map(this[target]);
    if (on) next.set(note, 100); else next.delete(note);
    this[target] = next;
  }

  padEvent(on: boolean, pad: number, session: number | undefined, port: string) {
    const verdict = classifyMidiSession(this.session, session, port);
    if (verdict === "stale") return;
    if (verdict === "newer") { this.session = session!; this.clearPhysical(); }
    const target = verdict === "virtual" ? "virtualPads" : "physicalPads";
    const next = new Map(this[target]);
    if (on) next.set(pad, 127); else next.delete(pad);
    this[target] = next;
  }
}

test("classifyMidiSession: virtual, stale, current, newer", () => {
  assert.equal(classifyMidiSession(5, undefined, "Virtual Input"), "virtual");
  assert.equal(classifyMidiSession(5, 0, "Nektar"), "virtual");
  assert.equal(classifyMidiSession(5, 4, "Nektar"), "stale");
  assert.equal(classifyMidiSession(5, 5, "Nektar"), "current");
  assert.equal(classifyMidiSession(5, 7, "Nektar"), "newer");
  assert.equal(isVirtualPortName("Virtual Pad"), true);
  assert.equal(isVirtualPortName("Nektar Impact LX49+"), false);
});

test("desconexión limpia teclas y pads físicos (caso 1 y 11)", () => {
  const s = new Sim();
  s.status(1);
  s.noteEvent(true, 60, 1, "Nektar");
  s.padEvent(true, 2, 1, "Nektar");
  s.noteEvent(true, 72, undefined, "Virtual Input"); // ratón
  s.padEvent(true, 5, undefined, "Virtual Pad"); // pad de la UI
  s.status(2); // desconexión (época nueva)
  assert.deepEqual([...s.notes.keys()], [72], "Solo queda la tecla virtual");
  assert.deepEqual([...s.pads.keys()], [5], "Solo queda el pad virtual");
});

test("eventos tardíos de la conexión anterior no reactivan controles (caso 3)", () => {
  const s = new Sim();
  s.status(1);
  s.status(2); // desconectado
  s.noteEvent(true, 60, 1, "Nektar"); // tardío
  s.padEvent(true, 1, 1, "Nektar");
  assert.equal(s.notes.size, 0);
  assert.equal(s.pads.size, 0);
});

test("reconexión del mismo dispositivo vuelve a funcionar (caso 4)", () => {
  const s = new Sim();
  s.status(1);
  s.noteEvent(true, 60, 1, "Nektar");
  s.status(2);
  s.status(3); // reconectado
  s.noteEvent(true, 60, 3, "Nektar");
  assert.deepEqual([...s.notes.keys()], [60]);
  s.noteEvent(false, 60, 3, "Nektar");
  assert.equal(s.notes.size, 0);
});

test("nota compartida ratón + MIDI: limpiar lo físico no apaga la virtual (caso 5 y 6)", () => {
  const s = new Sim();
  s.status(1);
  s.noteEvent(true, 60, 1, "Nektar");
  s.noteEvent(true, 60, undefined, "Virtual Input");
  s.padEvent(true, 1, 1, "Nektar");
  s.padEvent(true, 1, undefined, "Virtual Pad");
  s.status(2);
  assert.ok(s.notes.has(60));
  assert.ok(s.pads.has(1));
  s.noteEvent(false, 60, undefined, "Virtual Input");
  s.padEvent(false, 1, undefined, "Virtual Pad");
  assert.equal(s.notes.size, 0);
  assert.equal(s.pads.size, 0);
});

test("NoteOff físico no apaga una tecla que sostiene el ratón (A12)", () => {
  const s = new Sim();
  s.status(1);
  s.noteEvent(true, 60, undefined, "Virtual Input");
  s.noteEvent(true, 60, 1, "Nektar");
  s.noteEvent(false, 60, 1, "Nektar");
  assert.ok(s.notes.has(60));
});

test("cambios rápidos de puerto: solo gobierna la época más reciente (caso 12)", () => {
  const s = new Sim();
  for (let e = 1; e <= 10; e++) {
    s.status(e);
    s.noteEvent(true, 50 + e, e, "Puerto" + e);
  }
  // eventos tardíos de cualquier época anterior
  for (let e = 1; e < 10; e++) s.noteEvent(true, 100 + e, e, "Puerto" + e);
  assert.deepEqual([...s.notes.keys()], [60]);
  // un status-changed atrasado (época menor) no limpia ni retrocede
  s.status(4);
  assert.equal(s.session, 10);
  assert.deepEqual([...s.notes.keys()], [60]);
});

test("evento de una conexión nueva llega antes que su status-changed", () => {
  const s = new Sim();
  s.status(1);
  s.noteEvent(true, 60, 1, "A");
  s.noteEvent(true, 62, 3, "B"); // época 3 adoptada por el propio evento
  assert.deepEqual([...s.notes.keys()], [62], "Se descarta lo físico de la época 1");
  s.status(3); // llega después: sin efecto
  assert.deepEqual([...s.notes.keys()], [62]);
});
