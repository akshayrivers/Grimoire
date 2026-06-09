package EventLogger.queue;

import EventLogger.consumer.LogListener;
import EventLogger.model.Event;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;

public class LogDispatcher implements Runnable {

    private final EventQueue queue;
    private final List<LogListener> listeners = new ArrayList<>();

    public LogDispatcher(EventQueue queue) {
        this.queue = queue;
    }

    public void addListener(LogListener listener) {
        listeners.add(listener);
    }

    @Override
    public void run() {
        while (true) {
            try {
                Event event = queue.pop();
                for (LogListener listener : listeners) {
                    listener.onEvent(event);
                }
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
        }
    }
}
