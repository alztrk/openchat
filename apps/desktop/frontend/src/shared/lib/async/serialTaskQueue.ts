/** Runs tasks in order while keeping a failed task from blocking later work. */
export function createSerialTaskQueue() {
  let pendingTask: Promise<void> = Promise.resolve();

  return function enqueue<T>(task: () => Promise<T>): Promise<T> {
    const result = pendingTask.then(task, task);
    pendingTask = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  };
}
