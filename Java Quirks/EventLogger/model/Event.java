package EventLogger.model;

public class Event {

    private final String message;
    private final long timestamp;
    private final Severity severity;

    public Event(String message, Severity severity) {
        this.message = message;
        this.severity = severity;
        this.timestamp = System.currentTimeMillis();
    }

    public String getMessage() {
        return message;
    }

    public long getTimestamp() {
        return timestamp;
    }

    public Severity getSeverity() {
        return severity;
    }

    @Override
    public String toString() {
        return "[" +
                severity +
                "] " +
                timestamp +
                " : " +
                message;
    }
}
