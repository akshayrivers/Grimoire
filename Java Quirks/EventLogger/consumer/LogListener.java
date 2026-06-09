package EventLogger.consumer;

import EventLogger.model.Event;

public interface LogListener {
    void onEvent(Event event);
}
