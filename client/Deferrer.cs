internal static class Deferrer
{
    public static ImplAsync DeferAsync(Func<Task> action) => new (action);

    public struct ImplAsync : IAsyncDisposable
    {
        private readonly Func<Task> action;

        internal ImplAsync(Func<Task> action)
        {
            this.action = action;
        }

        public async ValueTask DisposeAsync()
        {
            await action();
        }
    }
}