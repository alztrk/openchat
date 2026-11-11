import { useToastStore } from '@shared/lib/toast/toastStore';
import { Toast } from './Toast';

export function ToastContainer() {
  const toasts = useToastStore((state) => state.toasts);
  const dismiss = useToastStore((state) => state.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div
      aria-atomic="false"
      aria-live="polite"
      className="pointer-events-none fixed top-12 right-6 z-50 flex w-full max-w-md flex-col items-end gap-2.5"
    >
      {toasts.map((item) => (
        <Toast
          action={item.action}
          className="pointer-events-auto w-full shadow-elevation-2"
          key={item.id}
          message={item.message}
          onClose={() => dismiss(item.id)}
          variant={item.variant}
        />
      ))}
    </div>
  );
}
