package EventLogger.consumer;

import EventLogger.model.Event;
import EventLogger.network.NetworkClient;

public class NetworkConsumer implements LogListener {

    private final NetworkClient client;

    public NetworkConsumer(NetworkClient client) {
        this.client = client;
    }

    @Override
    public void onEvent(Event e) {
        client.send(e);
    }
}
