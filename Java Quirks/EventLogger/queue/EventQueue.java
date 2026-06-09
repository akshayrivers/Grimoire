package EventLogger.queue;

import EventLogger.model.Event;

import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;

public class EventQueue {

    private final BlockingQueue<Event> queue = new LinkedBlockingQueue<>();

    public void push(Event e)
            throws InterruptedException {

        queue.put(e);
    }

    public Event pop()
            throws InterruptedException {

        return queue.take();
    }
}
