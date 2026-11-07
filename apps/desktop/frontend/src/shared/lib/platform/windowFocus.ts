export type AppWindowFocus = 'unknown' | 'focused' | 'backgrounded';

let appWindowFocus: AppWindowFocus = 'unknown';
let chatConversationSurfaceVisible = false;

export function setAppWindowFocus(focused: boolean): void {
  appWindowFocus = focused ? 'focused' : 'backgrounded';
}

export function resetAppWindowFocus(): void {
  appWindowFocus = 'unknown';
}

export function setChatConversationSurfaceVisible(visible: boolean): void {
  chatConversationSurfaceVisible = visible;
}

export function isAppWindowFocused(): boolean {
  return appWindowFocus === 'focused';
}

export function isAppWindowBackgrounded(): boolean {
  return appWindowFocus === 'backgrounded';
}

export function isAppWindowVisibleAndFocused(): boolean {
  return (
    chatConversationSurfaceVisible &&
    isAppWindowFocused() &&
    document.visibilityState === 'visible' &&
    document.hasFocus()
  );
}
