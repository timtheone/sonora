export type DictationMode = "push_to_toggle" | "push_to_talk";

export type DictationState =
  | "idle"
  | "listening"
  | "transcribing"
  | "inserting";

export type DictationEvent =
  | "hotkey_down"
  | "hotkey_up"
  | "speech_segment_ready"
  | "transcription_complete"
  | "insertion_complete"
  | "cancel";

export function transitionState(
  state: DictationState,
  mode: DictationMode,
  event: DictationEvent,
): DictationState {
  switch (state) {
    case "idle": {
      if (event === "hotkey_down") {
        return "listening";
      }
      return state;
    }
    case "listening": {
      if (event === "speech_segment_ready") {
        return "transcribing";
      }
      if (event === "cancel") {
        return "idle";
      }
      if (mode === "push_to_talk" && event === "hotkey_up") {
        return "idle";
      }
      if (mode === "push_to_toggle" && event === "hotkey_down") {
        return "idle";
      }
      return state;
    }
    case "transcribing": {
      if (event === "cancel") {
        return "idle";
      }
      if (event === "transcription_complete") {
        return "inserting";
      }
      return state;
    }
    case "inserting": {
      if (event === "cancel") {
        return "idle";
      }
      if (event === "insertion_complete") {
        return "idle";
      }
      return state;
    }
    default:
      return state;
  }
}
