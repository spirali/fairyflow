/**
 * Speaker notes for a scene attach to a "segment" — the span from the
 * previous cue (or scene start) up to the next cue (or scene end). Given the
 * scene's sorted cue frames, its (frame, text) notes, and a query frame, find
 * the notes belonging to the segment containing that frame.
 */
export function notesForSegment(
  cueFrames: number[],
  notes: [number, string][],
  frame: number,
  frameCount: number,
): string[] {
  let segmentStart = 0;
  let segmentEnd = frameCount;
  for (const cf of cueFrames) {
    if (cf <= frame) {
      segmentStart = Math.max(segmentStart, cf);
    } else {
      segmentEnd = Math.min(segmentEnd, cf);
    }
  }
  return notes.filter(([nf]) => nf >= segmentStart && nf < segmentEnd).map(([, text]) => text);
}
