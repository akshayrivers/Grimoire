package EventLogger.producer;

import EventLogger.model.Event;
import EventLogger.model.Severity;
import EventLogger.queue.EventQueue;

public class EventProducer implements Runnable {

    private final EventQueue queue;

    public EventProducer(EventQueue queue) {
        this.queue = queue;
    }

    @Override
    public void run() {

        int count = 1;

        while (true) {

            try {

                Event e = new Event(
                        "Event " + count++,
                        Severity.INFO);

                queue.push(e);

                System.out.println(
                        "Produced -> " + e);

                Thread.sleep(1000);

            } catch (Exception ex) {

                ex.printStackTrace();
            }
        }
    }
}
