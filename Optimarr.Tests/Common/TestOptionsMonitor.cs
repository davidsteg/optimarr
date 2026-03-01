using Microsoft.Extensions.Options;

namespace Optimarr.Tests.Common;

public sealed class TestOptionsMonitor<T> : IOptionsMonitor<T> where T : class
{
    private T _currentValue;

    public TestOptionsMonitor(T currentValue)
    {
        _currentValue = currentValue;
    }

    public T CurrentValue => _currentValue;

    public T Get(string? name) => _currentValue;

    public IDisposable OnChange(Action<T, string?> listener) => NoopDisposable.Instance;

    public void Set(T value)
    {
        _currentValue = value;
    }

    private sealed class NoopDisposable : IDisposable
    {
        public static readonly NoopDisposable Instance = new();
        public void Dispose()
        {
        }
    }
}
