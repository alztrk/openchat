export function ensureNoArguments(taskName, args) {
  if (args.length > 0) {
    throw new Error(`The ${taskName} task does not accept additional arguments.`);
  }
}

export function tryHelp(taskName, description, args, argsList) {
  if (argsList.length === 1 && (argsList[0] === '--help' || argsList[0] === '-h')) {
    process.stdout.write(`Usage: npm run ${taskName}${args.length > 0 ? ' [options]' : ''}\n`);
    process.stdout.write(`${description}.\n\n`);
    if (args.length > 0) {
      process.stdout.write('Options:\n');
      for (const arg of args) {
        const flag = arg.short ? `${arg.short}, ${arg.name}` : arg.name;
        const value = arg.value ? ` ${arg.value}` : '';
        process.stdout.write(`  ${flag}${value}  ${arg.description}\n`);
      }
    }
    process.stdout.write('\n');
    return true;
  }
  return false;
}

export function printHelp(tasks) {
  const taskNames = Object.keys(tasks).join('|');
  process.stdout.write(`Usage: npm run openchat -- <${taskNames}> [options]\n`);
  process.stdout.write('Run from apps/desktop/frontend. Use --help after a task for its options.\n');
}
