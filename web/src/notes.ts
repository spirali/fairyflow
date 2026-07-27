/**
 * Speaker notes for a scene attach to a "segment" — the span from the
 * previous cue (or scene start) up to *and including* the next cue (or
 * scene end). A cue only arms a lazy one-frame advance — content authored
 * after cue() doesn't visually change until the frame after it, and the
 * player itself pauses playback AT the cue frame — so while paused there,
 * the notes still showing should be the ones for the segment that just
 * played, not the one that hasn't started yet. Only once the frame moves
 * past the cue does the next segment's notes take over.
 *
 * Segment membership is tracked as an explicit ordinal (count of distinct
 * cue frames placed before the note), not inferred from frame number: a
 * note recorded right after a cue lands on the cue's own frame (cue()
 * doesn't flush the pending advance for note()), so a note from the
 * *previous* segment can share that exact frame number and would be
 * indistinguishable by a frame-range check alone.
 *
 * Given the scene's cue frames, its (frame, segment_ordinal, text) notes,
 * and a query frame, find the notes belonging to the segment containing
 * that frame. `cueFrames` need not be pre-sorted.
 */
export function notesForSegment(
  cueFrames: number[],
  notes: [number, number, string][],
  frame: number,
): string[] {
  const currentSegment = cueFrames.filter((cf) => cf < frame).length;
  return notes.filter(([, seg]) => seg === currentSegment).map(([, , text]) => text);
}
