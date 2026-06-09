package EventLogger;

import EventLogger.consumer.DatabaseConsumer;
import EventLogger.consumer.FileConsumer;
import EventLogger.consumer.NetworkConsumer;
import EventLogger.database.DatabaseManager;
import EventLogger.network.LogServer;
import EventLogger.network.NetworkClient;
import EventLogger.producer.EventProducer;
import EventLogger.queue.EventQueue;
import EventLogger.queue.LogDispatcher;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public class Main {

    public static void main(String[] args) {

        EventQueue queue = new EventQueue();
        LogDispatcher dispatcher = new LogDispatcher(queue);

        // Registering observers (listeners)
        dispatcher.addListener(new FileConsumer());
        dispatcher.addListener(new DatabaseConsumer(new DatabaseManager()));
        dispatcher.addListener(new NetworkConsumer(new NetworkClient()));

        ExecutorService executor = Executors.newFixedThreadPool(3);

        executor.execute(new LogServer());
        executor.execute(new EventProducer(queue));
        executor.execute(dispatcher);

        // to be implemented to handle closing gracefully
        // executor.shutdown();
    }
}
